//! Imports: loading a bank CSV file into one Account with that Account's
//! Import profile (CONTEXT.md, sync-design §1.5).
//!
//! - [`preview`] reads a file with a profile, without saving anything, so
//!   the user can see the parsed rows while setting the profile up.
//! - [`save_profile`] / [`profile_for_account`]: one Import profile per
//!   Account, synced like any other data.
//! - [`import_file`] turns the file's lines into Transactions with
//!   deterministic ids, skips the ones already present, and records the
//!   Import in the Account's history ([`history`]).
//!
//! The original file is only an input: it is never stored or synced.

mod parse;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow};
use crate::transactions::{Transaction, TxnSource};
use crate::{fulfilment, ids, manual_matches, rules, settings, transfers};

/// The text encoding of a bank's CSV files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "windows-1252")]
    Windows1252,
}

impl Encoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Windows1252 => "windows-1252",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "utf-8" => Ok(Encoding::Utf8),
            "windows-1252" => Ok(Encoding::Windows1252),
            other => Err(Error::Codec(format!("unknown encoding {other:?}"))),
        }
    }
}

/// How the bank writes amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmountMode {
    /// One column with a signed amount: negative = money out.
    Signed,
    /// A debit column (money out) and a credit column (money in).
    Split,
}

impl AmountMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AmountMode::Signed => "signed",
            AmountMode::Split => "split",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "signed" => Ok(AmountMode::Signed),
            "split" => Ok(AmountMode::Split),
            other => Err(Error::Codec(format!("unknown amount mode {other:?}"))),
        }
    }
}

/// The saved description of one bank's CSV layout for one Account (table
/// `import_profile`). Columns are named by their header text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProfile {
    /// Derived from the Account ([`profile_id`]): one profile per Account,
    /// with the same id on every Device.
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub encoding: Encoding,
    /// One character: `;`, `,` or a tab.
    pub delimiter: String,
    /// Lines before the header line.
    pub skip_rows: u32,
    pub date_column: String,
    /// strftime style, e.g. `%d/%m/%Y`.
    pub date_format: String,
    pub amount_mode: AmountMode,
    /// For [`AmountMode::Signed`].
    pub amount_column: Option<String>,
    /// For [`AmountMode::Split`]: money out.
    pub debit_column: Option<String>,
    /// For [`AmountMode::Split`]: money in.
    pub credit_column: Option<String>,
    /// `,` or `.`.
    pub decimal_sep: String,
    /// `.`, `,`, `'` or a space; `None` when the bank doesn't group thousands.
    pub thousands_sep: Option<String>,
    /// Joined with a space, in this order, to make the description.
    pub description_columns: Vec<String>,
    /// The balance after each line, if the bank includes it: checked against
    /// the calculated balance after an Import.
    pub balance_column: Option<String>,
}

impl ImportProfile {
    /// A starting point for a new profile for an Account: UTF-8, `;`,
    /// decimal comma, `%d/%m/%Y`, one signed amount column, no columns chosen.
    pub fn draft(account_id: &str) -> ImportProfile {
        ImportProfile {
            id: profile_id(account_id),
            account_id: account_id.to_owned(),
            name: String::new(),
            encoding: Encoding::Utf8,
            delimiter: ";".into(),
            skip_rows: 0,
            date_column: String::new(),
            date_format: "%d/%m/%Y".into(),
            amount_mode: AmountMode::Signed,
            amount_column: None,
            debit_column: None,
            credit_column: None,
            decimal_sep: ",".into(),
            thousands_sep: None,
            description_columns: Vec::new(),
            balance_column: None,
        }
    }
}

/// The id of an Account's Import profile. Deterministic, so a profile set up
/// on two Devices for the same Account is one row after Sync.
pub fn profile_id(account_id: &str) -> String {
    ids::derived(&format!("import_profile|{account_id}"))
}

impl SyncedRow for ImportProfile {
    const ENTITY: &'static str = "import_profile";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        let description_columns = serde_json::to_string(&self.description_columns)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT INTO import_profile (id, account_id, name, encoding, delimiter, skip_rows,
                 date_column, date_format, amount_mode, amount_column, debit_column, credit_column,
                 decimal_sep, thousands_sep, description_columns, balance_column, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 0)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id, name = excluded.name,
               encoding = excluded.encoding, delimiter = excluded.delimiter,
               skip_rows = excluded.skip_rows, date_column = excluded.date_column,
               date_format = excluded.date_format, amount_mode = excluded.amount_mode,
               amount_column = excluded.amount_column, debit_column = excluded.debit_column,
               credit_column = excluded.credit_column, decimal_sep = excluded.decimal_sep,
               thousands_sep = excluded.thousands_sep,
               description_columns = excluded.description_columns,
               balance_column = excluded.balance_column,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.account_id,
                self.name,
                self.encoding.as_str(),
                self.delimiter,
                self.skip_rows,
                self.date_column,
                self.date_format,
                self.amount_mode.as_str(),
                self.amount_column,
                self.debit_column,
                self.credit_column,
                self.decimal_sep,
                self.thousands_sep,
                description_columns,
                self.balance_column,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// One Import in an Account's history (table `import_batch`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportBatch {
    pub id: String,
    pub account_id: String,
    pub profile_id: String,
    /// The file name, e.g. `estratto_2026_10.csv`. The file itself is not kept.
    pub source_name: String,
    /// Unix ms.
    pub imported_at: i64,
    /// The earliest and latest Transaction dates in the file.
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    /// Transactions created by this Import.
    pub rows_new: i64,
    /// Lines skipped because their Transaction was already there.
    pub rows_skipped: i64,
}

impl SyncedRow for ImportBatch {
    const ENTITY: &'static str = "import_batch";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO import_batch (id, account_id, profile_id, source_name, imported_at,
                 first_date, last_date, rows_new, rows_skipped, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id, profile_id = excluded.profile_id,
               source_name = excluded.source_name, imported_at = excluded.imported_at,
               first_date = excluded.first_date, last_date = excluded.last_date,
               rows_new = excluded.rows_new, rows_skipped = excluded.rows_skipped,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.account_id,
                self.profile_id,
                self.source_name,
                self.imported_at,
                self.first_date,
                self.last_date,
                self.rows_new,
                self.rows_skipped,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// How many parsed rows a [`Preview`] shows.
pub const PREVIEW_ROWS: usize = 20;

/// A file as a profile reads it, for the setup step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    /// The first lines of the file as text, to choose the lines to skip.
    pub first_lines: Vec<String>,
    /// The header's column names, to choose the columns from.
    pub columns: Vec<String>,
    /// What stops the profile from reading the file at all (such as a
    /// missing column); when set, `rows` is empty.
    pub problem: Option<String>,
    /// The first [`PREVIEW_ROWS`] rows.
    pub rows: Vec<PreviewRow>,
    /// Every row in the file, and how many of them can't be read.
    pub total_rows: usize,
    pub rows_with_errors: usize,
}

/// One line of the file as it would be imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRow {
    /// The line number in the file.
    pub line: usize,
    pub date: Option<String>,
    pub amount_minor: Option<i64>,
    pub description: String,
    pub balance_after_minor: Option<i64>,
    /// Why this line can't be imported.
    pub error: Option<String>,
}

/// Read `file` with `profile` without saving anything.
pub fn preview(store: &Store, profile: &ImportProfile, file: &[u8]) -> Result<Preview> {
    let parsed = parse::parse(profile, minor_digits(store)?, file);
    Ok(Preview {
        first_lines: parsed.first_lines,
        columns: parsed.columns,
        problem: parsed.problem,
        total_rows: parsed.lines.len(),
        rows_with_errors: parsed.lines.iter().filter(|l| l.error.is_some()).count(),
        rows: parsed
            .lines
            .into_iter()
            .take(PREVIEW_ROWS)
            .map(|l| PreviewRow {
                line: l.line,
                date: l.date,
                amount_minor: l.amount_minor,
                description: l.description,
                balance_after_minor: l.balance_after_minor,
                error: l.error,
            })
            .collect(),
    })
}

/// The decimals of the Currency (EUR 2, JPY 0, ...). Amounts in a CSV are
/// converted to minor units with it.
fn minor_digits(store: &Store) -> Result<u32> {
    let currency = settings::currency(store)?.unwrap_or_default();
    Ok(match currency.as_str() {
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        _ => 2,
    })
}

/// The Account's Import profile, if it has one.
pub fn profile_for_account(store: &Store, account_id: &str) -> Result<Option<ImportProfile>> {
    find_profile(store.conn(), account_id)
}

/// Create or replace the Account's Import profile (its id is always
/// [`profile_id`] of the Account).
pub fn save_profile(store: &mut Store, profile: ImportProfile) -> Result<ImportProfile> {
    store.write(|w| {
        let account_name: Option<String> = w
            .conn()
            .query_row(
                "SELECT name FROM account WHERE id = ?1 AND deleted = 0",
                [&profile.account_id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(account_name) = account_name else {
            return Err(Error::invalid("Choose an existing Account."));
        };
        let profile = validated(profile, &account_name)?;
        w.upsert(&profile)?;
        Ok(profile)
    })
}

fn validated(mut p: ImportProfile, account_name: &str) -> Result<ImportProfile> {
    let trimmed = |s: Option<String>| s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    p.id = profile_id(&p.account_id);
    p.name = p.name.trim().to_owned();
    if p.name.is_empty() {
        p.name = format!("{account_name} CSV");
    }
    if p.delimiter.chars().count() != 1 || !p.delimiter.is_ascii() {
        return Err(Error::invalid(
            "The separator must be a single character such as ; or ,",
        ));
    }
    p.date_column = p.date_column.trim().to_owned();
    if p.date_column.is_empty() {
        return Err(Error::invalid("Choose the date column."));
    }
    parse::check_date_format(&p.date_format).map_err(Error::Invalid)?;
    p.amount_column = trimmed(p.amount_column);
    p.debit_column = trimmed(p.debit_column);
    p.credit_column = trimmed(p.credit_column);
    match p.amount_mode {
        AmountMode::Signed => {
            if p.amount_column.is_none() {
                return Err(Error::invalid("Choose the amount column."));
            }
            p.debit_column = None;
            p.credit_column = None;
        }
        AmountMode::Split => {
            if p.debit_column.is_none() || p.credit_column.is_none() {
                return Err(Error::invalid("Choose the debit and the credit columns."));
            }
            p.amount_column = None;
        }
    }
    if p.decimal_sep != "," && p.decimal_sep != "." {
        return Err(Error::invalid("The decimal separator must be , or ."));
    }
    p.thousands_sep = p.thousands_sep.filter(|t| !t.is_empty());
    if let Some(t) = &p.thousands_sep {
        if t.chars().count() != 1 || *t == p.decimal_sep {
            return Err(Error::invalid(
                "The thousands separator must be one character, different from the decimal separator.",
            ));
        }
    }
    p.description_columns = p
        .description_columns
        .into_iter()
        .map(|c| c.trim().to_owned())
        .filter(|c| !c.is_empty())
        .collect();
    if p.description_columns.is_empty() {
        return Err(Error::invalid("Choose at least one description column."));
    }
    p.balance_column = trimmed(p.balance_column);
    Ok(p)
}

fn find_profile(conn: &Connection, account_id: &str) -> Result<Option<ImportProfile>> {
    let row = conn
        .query_row(
            "SELECT id, account_id, name, encoding, delimiter, skip_rows, date_column, date_format,
                    amount_mode, amount_column, debit_column, credit_column, decimal_sep,
                    thousands_sep, description_columns, balance_column
             FROM import_profile WHERE id = ?1 AND deleted = 0",
            [profile_id(account_id)],
            |r| {
                Ok((
                    ImportProfile {
                        id: r.get(0)?,
                        account_id: r.get(1)?,
                        name: r.get(2)?,
                        encoding: Encoding::Utf8, // replaced below
                        delimiter: r.get(4)?,
                        skip_rows: r.get(5)?,
                        date_column: r.get(6)?,
                        date_format: r.get(7)?,
                        amount_mode: AmountMode::Signed, // replaced below
                        amount_column: r.get(9)?,
                        debit_column: r.get(10)?,
                        credit_column: r.get(11)?,
                        decimal_sep: r.get(12)?,
                        thousands_sep: r.get(13)?,
                        description_columns: Vec::new(), // replaced below
                        balance_column: r.get(15)?,
                    },
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(14)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(mut profile, encoding, amount_mode, description_columns)| {
            profile.encoding = Encoding::parse(&encoding)?;
            profile.amount_mode = AmountMode::parse(&amount_mode)?;
            profile.description_columns = serde_json::from_str(&description_columns)
                .map_err(|e| Error::Codec(e.to_string()))?;
            Ok(profile)
        },
    )
    .transpose()
}

/// What an Import did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSummary {
    /// The Import as recorded in the Account's history.
    pub batch: ImportBatch,
    /// When the profile has a balance column: the first line where the
    /// bank's balance disagrees with the balance the app calculates.
    pub balance_mismatch: Option<BalanceMismatch>,
}

/// A line of the file whose balance (after the last line of its day)
/// disagrees with the Account's calculated balance at the end of that day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceMismatch {
    /// The line number in the file.
    pub line: usize,
    pub date: String,
    pub description: String,
    /// The balance in the file.
    pub file_balance_minor: i64,
    /// The Opening balance plus the Account's Transactions up to that day.
    pub calculated_balance_minor: i64,
}

/// Import a bank CSV file into an Account with the Account's Import profile.
///
/// Each line becomes a Transaction whose id is derived from the Account,
/// date, amount, normalised description and an occurrence index for
/// identical lines on the same day (sync-design §1.5). Lines whose
/// Transaction already exists (even deleted) are skipped, so importing the
/// same or an overlapping file, on any Device, never creates duplicates.
///
/// The whole file is refused if any line can't be read. `source_name` is the
/// file name for the history; the file itself is not kept.
pub fn import_file(
    store: &mut Store,
    account_id: &str,
    source_name: &str,
    file: &[u8],
) -> Result<ImportSummary> {
    let profile = profile_for_account(store, account_id)?
        .ok_or_else(|| Error::invalid("Set up an Import profile for this Account first."))?;
    let parsed = parse::parse(&profile, minor_digits(store)?, file);
    if let Some(problem) = parsed.problem {
        return Err(Error::Invalid(problem));
    }
    if let Some(bad) = parsed.lines.iter().find(|l| l.error.is_some()) {
        return Err(Error::Invalid(format!(
            "Line {} can't be read: {} Nothing was imported.",
            bad.line,
            bad.error.as_deref().unwrap_or_default()
        )));
    }
    if parsed.lines.is_empty() {
        return Err(Error::invalid("The file has no Transactions to import."));
    }

    let batch_id = uuid::Uuid::now_v7().to_string();
    let mut occurrences: HashMap<(String, i64, String), u32> = HashMap::new();
    let candidates: Vec<Transaction> = parsed
        .lines
        .iter()
        .map(|line| {
            let date = line.date.clone().unwrap_or_default();
            let amount_minor = line.amount_minor.unwrap_or_default();
            let normalised = normalise(&line.description);
            let n = occurrences
                .entry((date.clone(), amount_minor, normalised.clone()))
                .or_insert(0);
            let fingerprint = fingerprint(account_id, &date, amount_minor, &normalised, *n);
            *n += 1;
            Transaction {
                id: ids::derived(&fingerprint),
                account_id: account_id.to_owned(),
                date,
                amount_minor,
                description: line.description.clone(),
                note: None,
                source: TxnSource::Import,
                import_id: Some(batch_id.clone()),
                fingerprint: Some(fingerprint),
                balance_after_minor: line.balance_after_minor,
                category_id: None,
                category_source: None,
            }
        })
        .collect();

    let batch = store.write(|w| {
        let account_live: bool = w.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM account WHERE id = ?1 AND deleted = 0)",
            [account_id],
            |r| r.get(0),
        )?;
        if !account_live {
            return Err(Error::invalid("Choose an existing Account."));
        }
        let rules = rules::applicable(w.conn())?;
        let (mut rows_new, mut rows_skipped) = (0, 0);
        let mut new_txns = Vec::new();
        let mut marked_as_transfer = Vec::new();
        for txn in &candidates {
            let exists: bool = w.conn().query_row(
                "SELECT EXISTS(SELECT 1 FROM txn WHERE id = ?1)",
                [&txn.id],
                |r| r.get(0),
            )?;
            if exists {
                rows_skipped += 1;
            } else {
                let mut txn = txn.clone();
                let rule = rules::categorise(&rules, &mut txn);
                if rule.is_some_and(|r| r.action == rules::RuleAction::Transfer) {
                    marked_as_transfer.push(txn.id.clone());
                }
                w.upsert(&txn)?;
                new_txns.push(txn);
                rows_new += 1;
            }
        }
        transfers::after_import(w, &new_txns, &marked_as_transfer)?;
        manual_matches::after_import(w, &new_txns)?;
        fulfilment::after_import(w, &new_txns)?;
        let batch = ImportBatch {
            id: batch_id.clone(),
            account_id: account_id.to_owned(),
            profile_id: profile.id.clone(),
            source_name: source_name.trim().to_owned(),
            imported_at: now_ms(),
            first_date: candidates.iter().map(|t| t.date.clone()).min(),
            last_date: candidates.iter().map(|t| t.date.clone()).max(),
            rows_new,
            rows_skipped,
        };
        w.upsert(&batch)?;
        Ok(batch)
    })?;
    let balance_mismatch = if profile.balance_column.is_some() {
        first_balance_mismatch(store.conn(), account_id, &parsed.lines)?
    } else {
        None
    };
    Ok(ImportSummary {
        batch,
        balance_mismatch,
    })
}

/// Compare the file's balance after the last line of each day with the
/// Account's calculated balance at the end of that day, oldest day first.
/// Days before the Opening balance date can't be checked.
fn first_balance_mismatch(
    conn: &Connection,
    account_id: &str,
    lines: &[parse::ParsedLine],
) -> Result<Option<BalanceMismatch>> {
    let (opening_minor, opening_date): (i64, String) = conn.query_row(
        "SELECT opening_minor, opening_date FROM account WHERE id = ?1",
        [account_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    // Banks write files oldest first or newest first; read them oldest first.
    let mut chronological: Vec<&parse::ParsedLine> = lines.iter().collect();
    if chronological.first().map(|l| &l.date) > chronological.last().map(|l| &l.date) {
        chronological.reverse();
    }
    let mut end_of_day: Vec<&parse::ParsedLine> = Vec::new();
    for line in chronological {
        match end_of_day.last_mut() {
            Some(last) if last.date == line.date => *last = line,
            _ => end_of_day.push(line),
        }
    }
    for line in end_of_day {
        let (Some(date), Some(file_balance_minor)) = (&line.date, line.balance_after_minor) else {
            continue;
        };
        if *date < opening_date {
            continue;
        }
        let after_opening: i64 = conn.query_row(
            &format!(
                "SELECT COALESCE(SUM(amount_minor), 0) FROM txn
                 WHERE account_id = ?1 AND deleted = 0 AND date > ?2 AND date <= ?3
                   AND id NOT IN ({})",
                manual_matches::HIDDEN_MANUAL_SQL
            ),
            params![account_id, opening_date, date],
            |r| r.get(0),
        )?;
        let calculated_balance_minor = opening_minor + after_opening;
        if calculated_balance_minor != file_balance_minor {
            return Ok(Some(BalanceMismatch {
                line: line.line,
                date: date.clone(),
                description: line.description.clone(),
                file_balance_minor,
                calculated_balance_minor,
            }));
        }
    }
    Ok(None)
}

/// The description as compared for duplicates: lower case, with runs of
/// white space made one space.
fn normalise(description: &str) -> String {
    description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `sha256(account_id | date | amount_minor | normalised description | n)`
/// in hex (sync-design §1.5).
fn fingerprint(
    account_id: &str,
    date: &str,
    amount_minor: i64,
    normalised: &str,
    n: u32,
) -> String {
    let digest = Sha256::digest(format!(
        "{account_id}|{date}|{amount_minor}|{normalised}|{n}"
    ));
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The Account's Imports, newest first.
pub fn history(store: &Store, account_id: &str) -> Result<Vec<ImportBatch>> {
    let mut stmt = store.conn().prepare(
        "SELECT id, account_id, profile_id, source_name, imported_at, first_date, last_date,
                rows_new, rows_skipped
         FROM import_batch WHERE account_id = ?1 AND deleted = 0
         ORDER BY imported_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([account_id], |r| {
        Ok(ImportBatch {
            id: r.get(0)?,
            account_id: r.get(1)?,
            profile_id: r.get(2)?,
            source_name: r.get(3)?,
            imported_at: r.get(4)?,
            first_date: r.get(5)?,
            last_date: r.get(6)?,
            rows_new: r.get(7)?,
            rows_skipped: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
