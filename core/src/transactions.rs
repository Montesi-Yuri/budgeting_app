//! Transactions: single movements of money in or out of one Account on a
//! date. This module covers Manual Transactions (entered by hand); Imports
//! (ticket 03) create the same rows with `source = Import`.
//!
//! Whether a Transaction is Income or an Expense is decided by the kind of
//! its Category, never by the sign of its amount (see [`crate::totals`]).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::categories::CategoryKind;
use crate::date;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow};

/// How the Transaction got into the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxnSource {
    Import,
    Manual,
}

impl TxnSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TxnSource::Import => "import",
            TxnSource::Manual => "manual",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "import" => Ok(TxnSource::Import),
            "manual" => Ok(TxnSource::Manual),
            other => Err(Error::Codec(format!(
                "unknown Transaction source {other:?}"
            ))),
        }
    }
}

/// Who set the Category. A Rule must never overwrite a Category set by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategorySource {
    User,
    Rule,
}

impl CategorySource {
    pub fn as_str(self) -> &'static str {
        match self {
            CategorySource::User => "user",
            CategorySource::Rule => "rule",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "user" => Ok(CategorySource::User),
            "rule" => Ok(CategorySource::Rule),
            other => Err(Error::Codec(format!("unknown Category source {other:?}"))),
        }
    }
}

/// One Transaction row (table `txn`). Amounts are signed minor units, as
/// seen from the Account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    /// `YYYY-MM-DD`; for a card purchase, the day of purchase.
    pub date: String,
    pub amount_minor: i64,
    pub description: String,
    pub note: Option<String>,
    pub source: TxnSource,
    /// The Import that created it (ticket 03).
    pub import_id: Option<String>,
    /// Imported rows only (sync-design §1.5).
    pub fingerprint: Option<String>,
    /// The balance after this line, if the bank's CSV has one.
    pub balance_after_minor: Option<i64>,
    /// `None` = Uncategorised.
    pub category_id: Option<String>,
    pub category_source: Option<CategorySource>,
}

impl SyncedRow for Transaction {
    const ENTITY: &'static str = "txn";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO txn (id, account_id, date, amount_minor, description, note, source,
                              import_id, fingerprint, balance_after_minor, category_id,
                              category_source, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id, date = excluded.date,
               amount_minor = excluded.amount_minor, description = excluded.description,
               note = excluded.note, source = excluded.source, import_id = excluded.import_id,
               fingerprint = excluded.fingerprint,
               balance_after_minor = excluded.balance_after_minor,
               category_id = excluded.category_id, category_source = excluded.category_source,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.account_id,
                self.date,
                self.amount_minor,
                self.description,
                self.note,
                self.source.as_str(),
                self.import_id,
                self.fingerprint,
                self.balance_after_minor,
                self.category_id,
                self.category_source.map(CategorySource::as_str),
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// What the user enters for a Manual Transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewManualTransaction {
    pub account_id: String,
    pub date: String,
    /// Signed: negative = money out of the Account.
    pub amount_minor: i64,
    pub description: String,
    pub category_id: Option<String>,
    pub note: Option<String>,
}

/// Add a Manual Transaction with a new UUIDv7 id. A Category given here is
/// set by the user.
pub fn create_manual(store: &mut Store, new: NewManualTransaction) -> Result<Transaction> {
    let txn = Transaction {
        id: uuid::Uuid::now_v7().to_string(),
        account_id: new.account_id,
        date: new.date,
        amount_minor: new.amount_minor,
        description: new.description,
        note: new.note,
        source: TxnSource::Manual,
        import_id: None,
        fingerprint: None,
        balance_after_minor: None,
        category_source: new.category_id.as_ref().map(|_| CategorySource::User),
        category_id: new.category_id,
    };
    store.write(|w| {
        let txn = validated(w.conn(), txn, None)?;
        w.upsert(&txn)?;
        Ok(txn)
    })
}

/// The fields the user can change on a Transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionEdit {
    pub id: String,
    pub account_id: String,
    pub date: String,
    pub amount_minor: i64,
    pub description: String,
    pub category_id: Option<String>,
    pub note: Option<String>,
}

/// Save the user's edit of a Transaction. A changed Category is marked as
/// set by the user; an unchanged one keeps who set it.
pub fn update(store: &mut Store, edit: TransactionEdit) -> Result<Transaction> {
    store.write(|w| {
        let current = find_live(w.conn(), &edit.id)?;
        let current_category = current.category_id.clone();
        let category_source = if edit.category_id == current.category_id {
            current.category_source
        } else {
            edit.category_id.as_ref().map(|_| CategorySource::User)
        };
        let txn = Transaction {
            account_id: edit.account_id,
            date: edit.date,
            amount_minor: edit.amount_minor,
            description: edit.description,
            note: edit.note,
            category_id: edit.category_id,
            category_source,
            ..current
        };
        let txn = validated(w.conn(), txn, Some(&current_category))?;
        w.upsert(&txn)?;
        Ok(txn)
    })
}

/// Set (or, with `None`, clear) a Transaction's Category by hand, e.g. from
/// the Uncategorised list. It is marked as set by the user, so a Rule will
/// never change it.
pub fn set_category(store: &mut Store, id: &str, category_id: Option<&str>) -> Result<Transaction> {
    store.write(|w| {
        let current = find_live(w.conn(), id)?;
        let txn = Transaction {
            category_id: category_id.map(str::to_owned),
            category_source: category_id.map(|_| CategorySource::User),
            ..current
        };
        let txn = validated(w.conn(), txn, None)?;
        w.upsert(&txn)?;
        Ok(txn)
    })
}

/// Delete a Transaction (a soft delete, so the deletion syncs).
pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.write(|w| w.delete::<Transaction>(id))?;
    Ok(())
}

/// Check what the user entered, and tidy the text fields. `unchanged_category`
/// is the Category the row already had: it is kept even if it has since been
/// deleted (e.g. on another Device).
fn validated(
    conn: &Connection,
    mut txn: Transaction,
    unchanged_category: Option<&Option<String>>,
) -> Result<Transaction> {
    let is_live = |table: &str, id: &str| -> Result<bool> {
        let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1 AND deleted = 0)");
        Ok(conn.query_row(&sql, [id], |r| r.get(0))?)
    };
    if !is_live("account", &txn.account_id)? {
        return Err(Error::invalid("Choose an existing Account."));
    }
    if !date::is_valid(&txn.date) {
        return Err(Error::invalid(
            "The date must be a valid date (YYYY-MM-DD).",
        ));
    }
    if txn.amount_minor == 0 {
        return Err(Error::invalid("Enter an amount other than zero."));
    }
    txn.description = txn.description.trim().to_owned();
    if txn.description.is_empty() {
        return Err(Error::invalid("A Transaction needs a description."));
    }
    txn.note = txn
        .note
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty());
    if let Some(category_id) = &txn.category_id {
        if unchanged_category != Some(&txn.category_id) && !is_live("category", category_id)? {
            return Err(Error::invalid("That Category doesn't exist any more."));
        }
    }
    Ok(txn)
}

fn find_live(conn: &Connection, id: &str) -> Result<Transaction> {
    list_where(conn, "t.id = ?1", params![id])?
        .pop()
        .map(|v| v.transaction)
        .ok_or_else(|| Error::NotFound {
            entity: Transaction::ENTITY,
            id: id.to_owned(),
        })
}

/// A Transaction as shown in a list: with its Account and Category names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionView {
    #[serde(flatten)]
    pub transaction: Transaction,
    pub account_name: String,
    /// `None` = Uncategorised, also when its Category has been deleted.
    pub category_name: Option<String>,
    /// The kind of its Category: whether it is Income or an Expense.
    pub category_kind: Option<CategoryKind>,
}

/// An Account's live Transactions, newest first (same day: last entered first).
pub fn list_for_account(store: &Store, account_id: &str) -> Result<Vec<TransactionView>> {
    list_where(store.conn(), "t.account_id = ?1", params![account_id])
}

/// Every live Transaction without a Category (or whose Category was
/// deleted), on any live Account, newest first.
pub fn list_uncategorised(store: &Store) -> Result<Vec<TransactionView>> {
    list_where(store.conn(), "c.id IS NULL", [])
}

/// Every live Transaction on any live Account, newest first.
pub(crate) fn list_live(conn: &Connection) -> Result<Vec<TransactionView>> {
    list_where(conn, "1 = 1", [])
}

const VIEW_SELECT: &str = "
    SELECT t.id, t.account_id, t.date, t.amount_minor, t.description, t.note, t.source,
           t.import_id, t.fingerprint, t.balance_after_minor, t.category_id, t.category_source,
           a.name, c.name, c.kind
    FROM txn t
    JOIN account a ON a.id = t.account_id AND a.deleted = 0
    LEFT JOIN category c ON c.id = t.category_id AND c.deleted = 0
    WHERE t.deleted = 0 AND ";

fn list_where(
    conn: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<TransactionView>> {
    let sql = format!("{VIEW_SELECT} {filter} ORDER BY t.date DESC, t.id DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(args, |r| {
        Ok((
            Transaction {
                id: r.get(0)?,
                account_id: r.get(1)?,
                date: r.get(2)?,
                amount_minor: r.get(3)?,
                description: r.get(4)?,
                note: r.get(5)?,
                source: TxnSource::Manual, // replaced below
                import_id: r.get(7)?,
                fingerprint: r.get(8)?,
                balance_after_minor: r.get(9)?,
                category_id: r.get(10)?,
                category_source: None, // replaced below
            },
            r.get::<_, String>(6)?,
            r.get::<_, Option<String>>(11)?,
            r.get::<_, String>(12)?,
            r.get::<_, Option<String>>(13)?,
            r.get::<_, Option<String>>(14)?,
        ))
    })?;
    rows.map(|row| {
        let (mut transaction, source, category_source, account_name, category_name, kind) = row?;
        transaction.source = TxnSource::parse(&source)?;
        transaction.category_source = category_source
            .as_deref()
            .map(CategorySource::parse)
            .transpose()?;
        Ok(TransactionView {
            transaction,
            account_name,
            category_name,
            category_kind: kind.as_deref().map(CategoryKind::parse).transpose()?,
        })
    })
    .collect()
}
