//! Fulfilling Planned items (CONTEXT.md "Planned item"): each occurrence is
//! first **expected** at the planned amount. When its actual Transaction
//! arrives the occurrence is **fulfilled** and the actual amount replaces
//! the planned one. One still expected after its Period has ended is marked
//! **skipped** or **arrives later** (with a new expected date) by the user.
//!
//! Only these decisions are stored, as [`PlannedOccurrence`] rows
//! (sync-design §1.3). The id is derived from the Planned item and the
//! schedule's due date ([`occurrence_id`]), so the same occurrence changed on
//! two Devices is one row. What is still expected is calculated by
//! [`crate::planned::expected_occurrences`], which reads these rows.
//!
//! - **Match:** after an Import the app suggests the occurrence a new
//!   Transaction fulfils ([`could_fulfil`]). A suggested fulfilment counts
//!   as confirmed until the user decides ([`confirm`] / [`unlink`]).
//! - **User:** [`link`] a Transaction to an occurrence by hand, [`unlink`]
//!   it, [`skip`] an occurrence, or say it [`arrives_later`].
//!
//! A rejected (unlinked) fulfilment is kept on the row (`link_status =
//! 'rejected'`, the occurrence expected again), so that Transaction is never
//! suggested for that occurrence again.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::ids;
use crate::planned::{self, Occurrence};
use crate::review::LinkStatus;
use crate::store::{Store, SyncedRow, Writer};
use crate::transactions::{self, Transaction, TransactionView};

/// What has happened to an occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OccurrenceState {
    /// Still expected on its due date. Stored only to remember a rejected
    /// fulfilment.
    Expected,
    /// Its actual Transaction is `txn_id` (unless `link_status` is
    /// rejected, which [`unlink`] never leaves in this state).
    Fulfilled,
    /// It didn't happen and won't.
    Skipped,
    /// It is expected on `later_date` instead.
    Later,
}

impl OccurrenceState {
    pub fn as_str(self) -> &'static str {
        match self {
            OccurrenceState::Expected => "expected",
            OccurrenceState::Fulfilled => "fulfilled",
            OccurrenceState::Skipped => "skipped",
            OccurrenceState::Later => "later",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "expected" => Ok(OccurrenceState::Expected),
            "fulfilled" => Ok(OccurrenceState::Fulfilled),
            "skipped" => Ok(OccurrenceState::Skipped),
            "later" => Ok(OccurrenceState::Later),
            other => Err(Error::Codec(format!("unknown occurrence state {other:?}"))),
        }
    }
}

/// One stored occurrence (table `planned_occurrence`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOccurrence {
    /// [`occurrence_id`] of the item and due date.
    pub id: String,
    pub planned_item_id: String,
    /// The date the schedule gives the occurrence (not moved).
    pub due_date: Date,
    pub state: OccurrenceState,
    /// The fulfilling Transaction, or the rejected one.
    pub txn_id: Option<String>,
    pub link_status: Option<LinkStatus>,
    /// Where an occurrence that arrives later is now expected.
    pub later_date: Option<Date>,
}

impl PlannedOccurrence {
    /// Expected on: the new date if it arrives later, else its due date.
    pub fn expected_date(&self) -> Date {
        self.later_date.unwrap_or(self.due_date)
    }

    /// Fulfilled by a Suggestion or a link that isn't rejected.
    pub fn is_fulfilled(&self) -> bool {
        self.state == OccurrenceState::Fulfilled && self.link_status != Some(LinkStatus::Rejected)
    }
}

impl SyncedRow for PlannedOccurrence {
    const ENTITY: &'static str = "planned_occurrence";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO planned_occurrence (id, planned_item_id, due_date, state, txn_id,
                                             link_status, later_date, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
             ON CONFLICT(id) DO UPDATE SET
               planned_item_id = excluded.planned_item_id, due_date = excluded.due_date,
               state = excluded.state, txn_id = excluded.txn_id,
               link_status = excluded.link_status, later_date = excluded.later_date,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.planned_item_id,
                self.due_date.to_string(),
                self.state.as_str(),
                self.txn_id,
                self.link_status.map(LinkStatus::as_str),
                self.later_date.map(|d| d.to_string()),
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// The id of the occurrence of `planned_item_id` due on `due_date` (the
/// schedule's date): UUIDv5, so it is the same on every Device.
pub fn occurrence_id(planned_item_id: &str, due_date: Date) -> String {
    ids::derived(&format!("planned_occurrence|{planned_item_id}|{due_date}"))
}

/// How many days from the expected date a fulfilling Transaction can be.
pub const MATCH_WINDOW_DAYS: i64 = 5;

/// How far (in percent of the planned amount) the actual amount can be.
pub const AMOUNT_TOLERANCE_PERCENT: i64 = 20;

/// Whether `txn` looks like the actual of `occurrence`: the same Account,
/// the same sign, an amount within ±20% of the planned one, dated at most
/// [`MATCH_WINDOW_DAYS`] from the date it is expected on.
pub fn could_fulfil(occurrence: &Occurrence, txn: &Transaction) -> bool {
    let Ok(date) = txn.date.parse::<Date>() else {
        return false;
    };
    let planned = occurrence.amount_minor;
    let actual = txn.amount_minor;
    txn.account_id == occurrence.account_id
        && planned.signum() == actual.signum()
        && planned != 0
        && (i128::from(actual) - i128::from(planned)).abs() * 100
            <= i128::from(planned).abs() * i128::from(AMOUNT_TOLERANCE_PERCENT)
        && occurrence.due_date.days_until(date).abs() <= MATCH_WINDOW_DAYS
}

/// After an Import, inside its write: suggest the occurrence each new
/// Transaction fulfils (sync-design §1.5 step 3). Transactions in a
/// Transfer are left alone. Of several expected occurrences the closest in
/// date wins, then the closest in amount. An occurrence fulfilled by one new
/// Transaction is no longer expected for the next, and a Transaction once
/// rejected for an occurrence is never suggested for it again.
pub(crate) fn after_import(w: &mut Writer<'_>, new_txns: &[Transaction]) -> Result<()> {
    let mut ordered: Vec<&Transaction> = new_txns.iter().collect();
    ordered.sort_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)));
    for txn in ordered {
        let Ok(date) = txn.date.parse::<Date>() else {
            continue;
        };
        if in_transfer(w.conn(), &txn.id)? || fulfilling(w.conn(), &txn.id)?.is_some() {
            continue;
        }
        let candidates = planned::expected_between(
            w.conn(),
            date.add_days(-MATCH_WINDOW_DAYS),
            date.add_days(MATCH_WINDOW_DAYS),
        )?;
        let mut best: Option<(i64, i64, Occurrence)> = None;
        for occurrence in candidates {
            if !could_fulfil(&occurrence, txn) {
                continue;
            }
            let stored = find(w.conn(), &occurrence.id)?;
            if stored.is_some_and(|s| {
                s.link_status == Some(LinkStatus::Rejected) && s.txn_id.as_ref() == Some(&txn.id)
            }) {
                continue;
            }
            let distance = occurrence.due_date.days_until(date).abs();
            let difference = (occurrence.amount_minor - txn.amount_minor).abs();
            let better = best.as_ref().is_none_or(|(bd, bdiff, b)| {
                (distance, difference, &occurrence.id) < (*bd, *bdiff, &b.id)
            });
            if better {
                best = Some((distance, difference, occurrence));
            }
        }
        if let Some((_, _, occurrence)) = best {
            let later_date = find(w.conn(), &occurrence.id)?.and_then(|s| s.later_date);
            w.upsert(&PlannedOccurrence {
                id: occurrence.id,
                planned_item_id: occurrence.planned_item_id,
                due_date: occurrence.scheduled_date,
                state: OccurrenceState::Fulfilled,
                txn_id: Some(txn.id.clone()),
                link_status: Some(LinkStatus::Suggested),
                later_date,
            })?;
        }
    }
    Ok(())
}

/// Link a Transaction to the occurrence of `planned_item_id` due on
/// `due_date` (the schedule's date) by hand, as confirmed. Any amount and
/// date is allowed. Fails if the Transaction already fulfils another
/// occurrence: unlink that one first.
pub fn link(
    store: &mut Store,
    planned_item_id: &str,
    due_date: Date,
    txn_id: &str,
) -> Result<PlannedOccurrence> {
    store.write(|w| {
        check_occurrence(w.conn(), planned_item_id, due_date)?;
        let txn = transactions::find_live(w.conn(), txn_id)?;
        let id = occurrence_id(planned_item_id, due_date);
        if fulfilling(w.conn(), &txn.id)?.is_some_and(|other| other.id != id) {
            return Err(Error::invalid(
                "This Transaction already fulfils another Planned item. Unlink it first.",
            ));
        }
        let row = PlannedOccurrence {
            state: OccurrenceState::Fulfilled,
            txn_id: Some(txn.id),
            link_status: Some(LinkStatus::Confirmed),
            ..stored_or_new(w.conn(), planned_item_id, due_date)?
        };
        w.upsert(&row)?;
        Ok(row)
    })
}

/// Confirm a suggested fulfilment (from the review list).
pub fn confirm(store: &mut Store, occurrence_id: &str) -> Result<PlannedOccurrence> {
    store.write(|w| {
        let row = find_fulfilled(w.conn(), occurrence_id)?;
        let row = PlannedOccurrence {
            link_status: Some(LinkStatus::Confirmed),
            ..row
        };
        w.upsert(&row)?;
        Ok(row)
    })
}

/// Reject a suggested fulfilment, or unlink a confirmed one: the
/// occurrence is expected again (on its new date if it arrives later), and
/// that Transaction is never suggested for it again.
pub fn unlink(store: &mut Store, occurrence_id: &str) -> Result<PlannedOccurrence> {
    store.write(|w| {
        let row = find_fulfilled(w.conn(), occurrence_id)?;
        let row = PlannedOccurrence {
            state: if row.later_date.is_some() {
                OccurrenceState::Later
            } else {
                OccurrenceState::Expected
            },
            link_status: Some(LinkStatus::Rejected),
            ..row
        };
        w.upsert(&row)?;
        Ok(row)
    })
}

/// Mark the occurrence of `planned_item_id` due on `due_date` as skipped:
/// it is no longer expected anywhere.
pub fn skip(store: &mut Store, planned_item_id: &str, due_date: Date) -> Result<PlannedOccurrence> {
    store.write(|w| {
        let row = PlannedOccurrence {
            state: OccurrenceState::Skipped,
            ..unfulfilled(w.conn(), planned_item_id, due_date)?
        };
        w.upsert(&row)?;
        Ok(row)
    })
}

/// The occurrence of `planned_item_id` due on `due_date` arrives later: it
/// is now expected on `new_date`, which must be after its due date (it may
/// be moved again).
pub fn arrives_later(
    store: &mut Store,
    planned_item_id: &str,
    due_date: Date,
    new_date: Date,
) -> Result<PlannedOccurrence> {
    store.write(|w| {
        if new_date <= due_date {
            return Err(Error::invalid(
                "The new expected date must be after the date it was due.",
            ));
        }
        let row = PlannedOccurrence {
            state: OccurrenceState::Later,
            later_date: Some(new_date),
            ..unfulfilled(w.conn(), planned_item_id, due_date)?
        };
        w.upsert(&row)?;
        Ok(row)
    })
}

/// The stored row of a live occurrence that isn't fulfilled, or a new one.
fn unfulfilled(
    conn: &Connection,
    planned_item_id: &str,
    due_date: Date,
) -> Result<PlannedOccurrence> {
    check_occurrence(conn, planned_item_id, due_date)?;
    let row = stored_or_new(conn, planned_item_id, due_date)?;
    if row.is_fulfilled() {
        return Err(Error::invalid(
            "This occurrence is fulfilled by a Transaction. Unlink it first.",
        ));
    }
    Ok(row)
}

fn stored_or_new(
    conn: &Connection,
    planned_item_id: &str,
    due_date: Date,
) -> Result<PlannedOccurrence> {
    let id = occurrence_id(planned_item_id, due_date);
    Ok(find(conn, &id)?.unwrap_or(PlannedOccurrence {
        id,
        planned_item_id: planned_item_id.to_owned(),
        due_date,
        state: OccurrenceState::Expected,
        txn_id: None,
        link_status: None,
        later_date: None,
    }))
}

/// Fails unless a live Planned item is due on `due_date` by its schedule.
fn check_occurrence(conn: &Connection, planned_item_id: &str, due_date: Date) -> Result<()> {
    let item = planned::find_live(conn, planned_item_id)?;
    if item.occurrences_between(due_date, due_date).is_empty() {
        return Err(Error::invalid("The Planned item isn't due on that date."));
    }
    Ok(())
}

fn find_fulfilled(conn: &Connection, id: &str) -> Result<PlannedOccurrence> {
    find(conn, id)?
        .filter(PlannedOccurrence::is_fulfilled)
        .ok_or_else(|| Error::NotFound {
            entity: PlannedOccurrence::ENTITY,
            id: id.to_owned(),
        })
}

fn in_transfer(conn: &Connection, txn_id: &str) -> Result<bool> {
    let sql = format!(
        "SELECT EXISTS(SELECT 1 WHERE ?1 IN ({}))",
        crate::transfers::IN_TRANSFER_SQL
    );
    Ok(conn.query_row(&sql, [txn_id], |r| r.get(0))?)
}

/// When a Manual Transaction is merged into its imported copy (ticket 07),
/// the occurrence it fulfils moves to the imported Transaction, so it isn't
/// expected again. Nothing changes if the Manual Transaction fulfils
/// nothing, or if the imported one already fulfils an occurrence.
pub(crate) fn hand_over(
    w: &mut Writer<'_>,
    manual_txn_id: &str,
    imported_txn_id: &str,
) -> Result<()> {
    let Some(row) = fulfilling(w.conn(), manual_txn_id)? else {
        return Ok(());
    };
    if fulfilling(w.conn(), imported_txn_id)?.is_some() {
        return Ok(());
    }
    w.upsert(&PlannedOccurrence {
        txn_id: Some(imported_txn_id.to_owned()),
        ..row
    })?;
    Ok(())
}

/// The occurrence `txn_id` fulfils (a link that isn't rejected), if any.
fn fulfilling(conn: &Connection, txn_id: &str) -> Result<Option<PlannedOccurrence>> {
    Ok(rows_where(
        conn,
        "state = 'fulfilled' AND link_status <> 'rejected' AND txn_id = ?1",
        [txn_id],
    )?
    .pop())
}

/// The live row `id`, if stored.
pub(crate) fn find(conn: &Connection, id: &str) -> Result<Option<PlannedOccurrence>> {
    Ok(rows_where(conn, "id = ?1", [id])?.pop())
}

/// Every live stored occurrence.
pub(crate) fn all(conn: &Connection) -> Result<Vec<PlannedOccurrence>> {
    rows_where(conn, "1 = 1", [])
}

/// Whether the fulfilling Transaction of `row` is live (so the occurrence
/// is settled). A fulfilment whose Transaction was deleted leaves the
/// occurrence expected again.
pub(crate) fn settled(conn: &Connection, row: &PlannedOccurrence) -> Result<bool> {
    Ok(match row.state {
        OccurrenceState::Skipped => true,
        OccurrenceState::Fulfilled if row.is_fulfilled() => match &row.txn_id {
            Some(txn_id) => transactions::view_live(conn, txn_id)?.is_some(),
            None => false,
        },
        _ => false,
    })
}

fn rows_where(
    conn: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<PlannedOccurrence>> {
    let sql = format!(
        "SELECT id, planned_item_id, due_date, state, txn_id, link_status, later_date
         FROM planned_occurrence WHERE deleted = 0 AND {filter} ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(args, |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (id, planned_item_id, due_date, state, txn_id, link_status, later_date) = row?;
        Ok(PlannedOccurrence {
            id,
            planned_item_id,
            due_date: due_date.parse()?,
            state: OccurrenceState::parse(&state)?,
            txn_id,
            link_status: link_status.as_deref().map(LinkStatus::parse).transpose()?,
            later_date: later_date.map(|d| d.parse()).transpose()?,
        })
    })
    .collect()
}

/// The ids of the Transactions that fulfil an occurrence of a live Planned
/// item spread as Set-asides (links not rejected), as a SQL sub-query.
/// Spendable takes them back out: the Set-asides already cover them.
pub(crate) const FULFILLING_SET_ASIDE_SQL: &str = "
    SELECT o.txn_id FROM planned_occurrence o
    JOIN planned_item p ON p.id = o.planned_item_id AND p.deleted = 0 AND p.set_aside = 1
    WHERE o.deleted = 0 AND o.state = 'fulfilled' AND o.link_status <> 'rejected'
      AND o.txn_id IS NOT NULL";

/// A fulfilled occurrence as shown in the app: the Planned item next to
/// its actual Transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fulfilment {
    /// The occurrence id.
    pub id: String,
    pub planned_item_id: String,
    /// The Planned item's description.
    pub description: String,
    /// The schedule's date for the occurrence.
    pub due_date: Date,
    /// The planned amount the actual one replaces.
    pub planned_amount_minor: i64,
    /// Suggested or confirmed.
    pub status: LinkStatus,
    pub transaction: TransactionView,
}

/// Every fulfilled occurrence of a live Planned item with a live
/// Transaction, oldest first.
pub fn list(store: &Store) -> Result<Vec<Fulfilment>> {
    let conn = store.conn();
    let mut out = Vec::new();
    for row in all(conn)?
        .into_iter()
        .filter(PlannedOccurrence::is_fulfilled)
    {
        let (Some(txn_id), Some(status)) = (&row.txn_id, row.link_status) else {
            continue;
        };
        let Some(transaction) = transactions::view_live(conn, txn_id)? else {
            continue;
        };
        let Ok(item) = planned::find_live(conn, &row.planned_item_id) else {
            continue;
        };
        out.push(Fulfilment {
            id: row.id,
            planned_item_id: item.id,
            description: item.description,
            due_date: row.due_date,
            planned_amount_minor: item.amount_minor,
            status,
            transaction,
        });
    }
    out.sort_by(|a, b| (a.due_date, &a.id).cmp(&(b.due_date, &b.id)));
    Ok(out)
}
