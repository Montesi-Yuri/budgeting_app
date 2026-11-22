//! Matching Manual Transactions on Import (CONTEXT.md "Manual Transaction",
//! sync-design §1.5 step 3 and §1.6).
//!
//! When an Import brings in a Transaction the user already entered by hand,
//! the app suggests linking the two: a `manual_match` row with a
//! [`LinkStatus`]. While the match is suggested or confirmed, only the
//! imported Transaction counts: the Manual Transaction is hidden from every
//! number ([`HIDDEN_MANUAL_SQL`]). Confirming merges them ([`merged`]): the
//! imported amount and date win, and the Manual Transaction's Category (as
//! set by the user) and note move onto the imported one; the Manual
//! Transaction is soft-deleted. Rejecting keeps both, and the pair is never
//! suggested again.
//!
//! The id is derived from the pair ([`match_id`]), so the same match made on
//! two Devices is one row after Sync.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::ids;
use crate::review::LinkStatus;
use crate::store::{Store, SyncedRow, Writer};
use crate::transactions::{self, CategorySource, Transaction, TransactionView, TxnSource};
use crate::{fulfilment, transfers};

/// One match between a Manual Transaction and an imported Transaction
/// (table `manual_match`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualMatch {
    pub id: String,
    /// The Transaction the user entered by hand.
    pub manual_txn_id: String,
    /// The Transaction that arrived in an Import.
    pub imported_txn_id: String,
    pub status: LinkStatus,
}

impl SyncedRow for ManualMatch {
    const ENTITY: &'static str = "manual_match";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO manual_match (id, manual_txn_id, imported_txn_id, status, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)
             ON CONFLICT(id) DO UPDATE SET
               manual_txn_id = excluded.manual_txn_id,
               imported_txn_id = excluded.imported_txn_id,
               status = excluded.status, updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.manual_txn_id,
                self.imported_txn_id,
                self.status.as_str(),
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// The id of the match between these two Transactions: UUIDv5 of the pair
/// (sync-design §1.3), so the same match is one row on every Device.
pub fn match_id(manual_txn_id: &str, imported_txn_id: &str) -> String {
    ids::derived(&format!("manual-match|{manual_txn_id}|{imported_txn_id}"))
}

/// The ids of the Manual Transactions hidden by a match, as a SQL
/// sub-query: `t.id NOT IN (...)` leaves them out of the numbers. A match
/// hides its Manual Transaction while it isn't rejected and its imported
/// Transaction is still there.
pub(crate) const HIDDEN_MANUAL_SQL: &str = "
    SELECT m.manual_txn_id FROM manual_match m
    JOIN txn i ON i.id = m.imported_txn_id AND i.deleted = 0
    WHERE m.deleted = 0 AND m.status <> 'rejected'";

/// How many days apart a Manual Transaction and its imported Transaction
/// can be.
pub const MATCH_WINDOW_DAYS: i64 = 4;

/// How far apart the two amounts can be, in minor units (for rounding).
pub const AMOUNT_TOLERANCE_MINOR: i64 = 1;

/// Whether `manual` (entered by hand) looks like the same Transaction as
/// `imported`: on the same Account, with the same sign and the same amount
/// give or take [`AMOUNT_TOLERANCE_MINOR`], dated at most
/// [`MATCH_WINDOW_DAYS`] apart.
pub fn could_match(manual: &Transaction, imported: &Transaction) -> bool {
    let (Ok(manual_date), Ok(imported_date)) =
        (manual.date.parse::<Date>(), imported.date.parse::<Date>())
    else {
        return false;
    };
    manual.source == TxnSource::Manual
        && imported.source == TxnSource::Import
        && manual.account_id == imported.account_id
        && manual.amount_minor.signum() == imported.amount_minor.signum()
        && (manual.amount_minor - imported.amount_minor).abs() <= AMOUNT_TOLERANCE_MINOR
        && manual_date.days_until(imported_date).abs() <= MATCH_WINDOW_DAYS
}

/// The imported Transaction after a confirmed match (sync-design §1.6): its
/// own amount, date and description, with the Manual Transaction's Category
/// (marked as set by the user, so a Rule never changes it) and note. A
/// Manual Transaction without a Category or note leaves the imported
/// one's in place.
pub fn merged(manual: &Transaction, imported: &Transaction) -> Transaction {
    let mut merged = imported.clone();
    if manual.category_id.is_some() {
        merged.category_id = manual.category_id.clone();
        merged.category_source = Some(CategorySource::User);
    }
    if manual.note.is_some() {
        merged.note = manual.note.clone();
    }
    merged
}

/// After an Import, inside its write: suggest a match for every new
/// Transaction that has one (sync-design §1.5 step 3). The candidate is a
/// live Manual Transaction that [`could_match`] it, isn't already matched,
/// and was never rejected as a match with it. When several could, the
/// closest amount wins, then the closest date.
pub(crate) fn after_import(w: &mut Writer<'_>, new_txns: &[Transaction]) -> Result<()> {
    let mut ordered: Vec<&Transaction> = new_txns.iter().collect();
    ordered.sort_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)));
    for imported in ordered {
        if let Some(manual) = best_manual(w.conn(), imported)? {
            w.upsert(&ManualMatch {
                id: match_id(&manual.id, &imported.id),
                manual_txn_id: manual.id,
                imported_txn_id: imported.id.clone(),
                status: LinkStatus::Suggested,
            })?;
        }
    }
    Ok(())
}

fn best_manual(conn: &Connection, imported: &Transaction) -> Result<Option<Transaction>> {
    let Ok(date) = imported.date.parse::<Date>() else {
        return Ok(None);
    };
    let filter = format!(
        "t.source = 'manual' AND t.account_id = ?1
         AND t.amount_minor BETWEEN ?2 AND ?3 AND t.date BETWEEN ?4 AND ?5
         AND t.id NOT IN ({HIDDEN_MANUAL_SQL})"
    );
    let candidates = transactions::list_where(
        conn,
        &filter,
        params![
            imported.account_id,
            imported.amount_minor - AMOUNT_TOLERANCE_MINOR,
            imported.amount_minor + AMOUNT_TOLERANCE_MINOR,
            date.add_days(-MATCH_WINDOW_DAYS).to_string(),
            date.add_days(MATCH_WINDOW_DAYS).to_string()
        ],
    )?;
    let mut best: Option<((i64, i64, String), Transaction)> = None;
    for view in candidates {
        let manual = view.transaction;
        if !could_match(&manual, imported)
            || was_rejected(conn, &match_id(&manual.id, &imported.id))?
        {
            continue;
        }
        let distance = manual
            .date
            .parse::<Date>()
            .map_or(i64::MAX, |d| d.days_until(date).abs());
        let key = (
            (manual.amount_minor - imported.amount_minor).abs(),
            distance,
            manual.id.clone(),
        );
        if best.as_ref().is_none_or(|(best_key, _)| key < *best_key) {
            best = Some((key, manual));
        }
    }
    Ok(best.map(|(_, manual)| manual))
}

fn was_rejected(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM manual_match WHERE id = ?1 AND status = 'rejected')",
        [id],
        |r| r.get(0),
    )?)
}

/// A match as shown in the review list: both versions side by side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManualMatchView {
    #[serde(flatten)]
    pub manual_match: ManualMatch,
    /// The version entered by hand.
    pub manual_txn: TransactionView,
    /// The version from the Import.
    pub imported_txn: TransactionView,
}

/// The suggested matches whose two Transactions are both still there,
/// oldest first (by the imported Transaction's date).
pub fn suggestions(store: &Store) -> Result<Vec<ManualMatchView>> {
    let conn = store.conn();
    let mut views = Vec::new();
    for manual_match in matches_where(conn, "status = 'suggested'", [])? {
        let (Some(manual_txn), Some(imported_txn)) = (
            transactions::view_live(conn, &manual_match.manual_txn_id)?,
            transactions::view_live(conn, &manual_match.imported_txn_id)?,
        ) else {
            continue;
        };
        views.push(ManualMatchView {
            manual_match,
            manual_txn,
            imported_txn,
        });
    }
    views.sort_by(|a, b| {
        (&a.imported_txn.transaction.date, &a.manual_match.id)
            .cmp(&(&b.imported_txn.transaction.date, &b.manual_match.id))
    });
    Ok(views)
}

/// The live match rows matching `filter`.
fn matches_where(
    conn: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<ManualMatch>> {
    let sql = format!(
        "SELECT id, manual_txn_id, imported_txn_id, status FROM manual_match
         WHERE deleted = 0 AND {filter} ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(args, |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    rows.map(|row| {
        let (id, manual_txn_id, imported_txn_id, status) = row?;
        Ok(ManualMatch {
            id,
            manual_txn_id,
            imported_txn_id,
            status: LinkStatus::parse(&status)?,
        })
    })
    .collect()
}

/// Confirm a suggested match: the two Transactions merge into the imported
/// one ([`merged`]), and the Manual Transaction is soft-deleted. If the
/// Manual Transaction was a side of a Transfer, the imported one takes its
/// place there. Each step
/// is a normal row write, so the merge syncs like any other change.
pub fn confirm(store: &mut Store, match_id: &str) -> Result<ManualMatch> {
    store.write(|w| {
        let manual_match = find_suggested(w.conn(), match_id)?;
        let manual = transactions::find_live(w.conn(), &manual_match.manual_txn_id)?;
        let imported = transactions::find_live(w.conn(), &manual_match.imported_txn_id)?;
        let merged = merged(&manual, &imported);
        w.upsert(&merged)?;
        transfers::hand_over_side(w, &manual.id, &merged)?;
        fulfilment::hand_over(w, &manual.id, &merged.id)?;
        w.delete::<Transaction>(&manual.id)?;
        let manual_match = ManualMatch {
            status: LinkStatus::Confirmed,
            ..manual_match
        };
        w.upsert(&manual_match)?;
        Ok(manual_match)
    })
}

/// The live match `id`, which must still be waiting for a decision.
fn find_suggested(conn: &Connection, id: &str) -> Result<ManualMatch> {
    let manual_match =
        matches_where(conn, "id = ?1", [id])?
            .pop()
            .ok_or_else(|| Error::NotFound {
                entity: ManualMatch::ENTITY,
                id: id.to_owned(),
            })?;
    if manual_match.status != LinkStatus::Suggested {
        return Err(Error::invalid("This match has already been decided."));
    }
    Ok(manual_match)
}

/// Reject a suggested match: both Transactions stay and count again, and
/// the row is kept as `rejected`, so the same pair is never suggested again.
pub fn reject(store: &mut Store, match_id: &str) -> Result<ManualMatch> {
    store.write(|w| {
        let manual_match = ManualMatch {
            status: LinkStatus::Rejected,
            ..find_suggested(w.conn(), match_id)?
        };
        w.upsert(&manual_match)?;
        Ok(manual_match)
    })
}
