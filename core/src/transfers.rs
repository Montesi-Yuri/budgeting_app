//! Transfers: money moved between two of the user's own Accounts, made of
//! an outgoing and an incoming Transaction (CONTEXT.md "Transfer"). Paying
//! the credit card bill is a Transfer. A Transfer is never Income or an
//! Expense: [`crate::totals`] leaves out every Transaction in a Transfer
//! that isn't rejected, so Spendable and Budgets do too.
//!
//! A Transfer is a row with a [`LinkStatus`] (sync-design §1.3):
//!
//! - **Rule:** a Rule with the action "mark as Transfer" makes a confirmed
//!   Transfer on Import, with only one side if the other side is missing.
//! - **Match:** after an Import the app suggests pairs ([`could_pair`]).
//!   A Transfer with only one side is completed when its other side arrives.
//! - **User:** [`link`] two Transactions by hand; [`reject`] unlinks.
//!
//! Ids are derived (UUIDv5), so two Devices that create the same Transfer
//! write the same row and Sync merges them: [`pair_id`] for a Transfer with
//! both sides, [`side_id`] for one with only one side so far.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::ids;
use crate::period::Period;
use crate::review::LinkStatus;
use crate::settings;
use crate::store::{Store, SyncedRow, Writer};
use crate::transactions::{self, Transaction, TransactionView};

/// Who made the Transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferOrigin {
    /// A Rule with the action "mark as Transfer".
    Rule,
    /// Suggested by the app after an Import.
    Match,
    /// Linked by the user by hand.
    User,
}

impl TransferOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            TransferOrigin::Rule => "rule",
            TransferOrigin::Match => "match",
            TransferOrigin::User => "user",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "rule" => Ok(TransferOrigin::Rule),
            "match" => Ok(TransferOrigin::Match),
            "user" => Ok(TransferOrigin::User),
            other => Err(Error::Codec(format!("unknown Transfer origin {other:?}"))),
        }
    }
}

/// One Transfer row (table `transfer`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    pub id: String,
    /// The outgoing Transaction (negative amount); `None` until imported.
    pub out_txn_id: Option<String>,
    /// The incoming Transaction (positive amount); `None` until imported.
    pub in_txn_id: Option<String>,
    pub status: LinkStatus,
    pub origin: TransferOrigin,
}

impl SyncedRow for Transfer {
    const ENTITY: &'static str = "transfer";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO transfer (id, out_txn_id, in_txn_id, status, origin, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
             ON CONFLICT(id) DO UPDATE SET
               out_txn_id = excluded.out_txn_id, in_txn_id = excluded.in_txn_id,
               status = excluded.status, origin = excluded.origin,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.out_txn_id,
                self.in_txn_id,
                self.status.as_str(),
                self.origin.as_str(),
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// The id of the Transfer made of these two Transactions: UUIDv5 of the
/// pair (sync-design §1.3), so the same pair is one row on every Device.
pub fn pair_id(out_txn_id: &str, in_txn_id: &str) -> String {
    ids::derived(&format!("transfer|{out_txn_id}|{in_txn_id}"))
}

/// The id of a Transfer that so far has only this one side.
pub fn side_id(txn_id: &str) -> String {
    ids::derived(&format!("transfer-side|{txn_id}"))
}

/// How many days apart the two sides of a suggested Transfer can be.
pub const MATCH_WINDOW_DAYS: i64 = 3;

/// Whether `a` and `b` look like the two sides of one Transfer: the same
/// absolute amount with opposite signs, on different Accounts, dated at
/// most [`MATCH_WINDOW_DAYS`] apart (either way round).
pub fn could_pair(a: &Transaction, b: &Transaction) -> bool {
    let (Ok(a_date), Ok(b_date)) = (a.date.parse::<Date>(), b.date.parse::<Date>()) else {
        return false;
    };
    a.account_id != b.account_id
        && a.amount_minor != 0
        && a.amount_minor == -b.amount_minor
        && a_date.days_until(b_date).abs() <= MATCH_WINDOW_DAYS
}

/// The ids of the Transactions in a Transfer that isn't rejected, as a SQL
/// sub-query: `t.id NOT IN (...)` leaves Transfers out of the numbers.
pub(crate) const IN_TRANSFER_SQL: &str = "
    SELECT out_txn_id FROM transfer
    WHERE deleted = 0 AND status <> 'rejected' AND out_txn_id IS NOT NULL
    UNION
    SELECT in_txn_id FROM transfer
    WHERE deleted = 0 AND status <> 'rejected' AND in_txn_id IS NOT NULL";

/// Link two Transactions as a Transfer by hand (confirmed). One must be
/// money out and the other money in, on different Accounts; their amounts
/// may differ. A Transfer that so far has only one of them as its side is
/// completed by this link. Fails if either is already in a Transfer with
/// both sides: unlink that one first.
pub fn link(store: &mut Store, a_txn_id: &str, b_txn_id: &str) -> Result<Transfer> {
    store.write(|w| {
        let a = transactions::find_live(w.conn(), a_txn_id)?;
        let b = transactions::find_live(w.conn(), b_txn_id)?;
        if a.account_id == b.account_id {
            return Err(Error::invalid(
                "A Transfer moves money between two different Accounts.",
            ));
        }
        let (out, into) = match (a.amount_minor < 0, b.amount_minor < 0) {
            (true, false) => (a, b),
            (false, true) => (b, a),
            _ => {
                return Err(Error::invalid(
                    "A Transfer needs one Transaction with money out and one with money in.",
                ))
            }
        };
        for txn in [&out, &into] {
            if let Some(existing) = live_transfer_of(w.conn(), &txn.id)? {
                if existing.out_txn_id.is_some() && existing.in_txn_id.is_some() {
                    return Err(Error::invalid(
                        "One of these Transactions is already part of a Transfer. Unlink it first.",
                    ));
                }
                w.delete::<Transfer>(&existing.id)?;
            }
        }
        let transfer = Transfer {
            id: pair_id(&out.id, &into.id),
            out_txn_id: Some(out.id),
            in_txn_id: Some(into.id),
            status: LinkStatus::Confirmed,
            origin: TransferOrigin::User,
        };
        w.upsert(&transfer)?;
        Ok(transfer)
    })
}

/// After an Import, inside its write: give every new Transaction its
/// Transfer, if it has one (sync-design §1.5 steps 2 and 3).
///
/// - Transactions in `marked_by_rule` (matched by a "mark as Transfer"
///   Rule) become a confirmed Transfer: with their other side if it is
///   found, otherwise with one side only.
/// - Any other new Transaction whose other side is found ([`could_pair`],
///   not already in a Transfer, never rejected as a pair) becomes a
///   suggested Transfer, which counts as confirmed until the user decides.
/// - An other side that so far is a Transfer on its own is completed: the
///   pair keeps that Transfer's status and origin.
///
/// When several Transactions could be the other side, the closest in date
/// wins.
pub(crate) fn after_import(
    w: &mut Writer<'_>,
    new_txns: &[Transaction],
    marked_by_rule: &[String],
) -> Result<()> {
    let mut ordered: Vec<&Transaction> = new_txns.iter().collect();
    ordered.sort_by(|a, b| (&a.date, &a.id).cmp(&(&b.date, &b.id)));
    for txn in ordered {
        if live_transfer_of(w.conn(), &txn.id)?.is_some() {
            continue;
        }
        let by_rule = marked_by_rule.contains(&txn.id);
        let transfer = match other_side(w.conn(), txn)? {
            Some((other, alone)) => {
                let (status, origin) = match (&alone, by_rule) {
                    (_, true) => (LinkStatus::Confirmed, TransferOrigin::Rule),
                    (Some(alone), false) => (alone.status, alone.origin),
                    (None, false) => (LinkStatus::Suggested, TransferOrigin::Match),
                };
                if let Some(alone) = alone {
                    w.delete::<Transfer>(&alone.id)?;
                }
                let (out, into) = if txn.amount_minor < 0 {
                    (txn, &other)
                } else {
                    (&other, txn)
                };
                Transfer {
                    id: pair_id(&out.id, &into.id),
                    out_txn_id: Some(out.id.clone()),
                    in_txn_id: Some(into.id.clone()),
                    status,
                    origin,
                }
            }
            None if by_rule => one_sided(txn, LinkStatus::Confirmed, TransferOrigin::Rule),
            None => continue,
        };
        w.upsert(&transfer)?;
    }
    Ok(())
}

/// A Transfer with only `txn` as its side so far.
fn one_sided(txn: &Transaction, status: LinkStatus, origin: TransferOrigin) -> Transfer {
    let (out_txn_id, in_txn_id) = if txn.amount_minor < 0 {
        (Some(txn.id.clone()), None)
    } else {
        (None, Some(txn.id.clone()))
    };
    Transfer {
        id: side_id(&txn.id),
        out_txn_id,
        in_txn_id,
        status,
        origin,
    }
}

/// The best other side for `txn`: a live Transaction that [`could_pair`]
/// with it, is in no Transfer (or is a Transfer on its own, returned too),
/// and was never rejected as a pair with it. The closest in date wins.
fn other_side(
    conn: &Connection,
    txn: &Transaction,
) -> Result<Option<(Transaction, Option<Transfer>)>> {
    let Ok(date) = txn.date.parse::<Date>() else {
        return Ok(None);
    };
    let candidates = transactions::list_where(
        conn,
        "t.amount_minor = ?1 AND t.account_id <> ?2 AND t.date BETWEEN ?3 AND ?4",
        params![
            -txn.amount_minor,
            txn.account_id,
            date.add_days(-MATCH_WINDOW_DAYS).to_string(),
            date.add_days(MATCH_WINDOW_DAYS).to_string()
        ],
    )?;
    let mut best: Option<(i64, Transaction, Option<Transfer>)> = None;
    for view in candidates {
        let other = view.transaction;
        if !could_pair(txn, &other) {
            continue;
        }
        let alone = match live_transfer_of(conn, &other.id)? {
            Some(t) if t.out_txn_id.is_some() && t.in_txn_id.is_some() => continue,
            alone => alone,
        };
        let (out, into) = if txn.amount_minor < 0 {
            (&txn.id, &other.id)
        } else {
            (&other.id, &txn.id)
        };
        if was_rejected(conn, &pair_id(out, into))? {
            continue;
        }
        let distance = other
            .date
            .parse::<Date>()
            .map_or(i64::MAX, |d| d.days_until(date).abs());
        let better = best.as_ref().is_none_or(|(best_distance, best_txn, _)| {
            (distance, &other.id) < (*best_distance, &best_txn.id)
        });
        if better {
            best = Some((distance, other, alone));
        }
    }
    Ok(best.map(|(_, other, alone)| (other, alone)))
}

fn was_rejected(conn: &Connection, transfer_id: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM transfer WHERE id = ?1 AND status = 'rejected')",
        [transfer_id],
        |r| r.get(0),
    )?)
}

/// Confirm a Transfer (e.g. a Suggestion from the review list).
pub fn confirm(store: &mut Store, transfer_id: &str) -> Result<Transfer> {
    store.write(|w| {
        let transfer = Transfer {
            status: LinkStatus::Confirmed,
            ..find_live(w.conn(), transfer_id)?
        };
        w.upsert(&transfer)?;
        Ok(transfer)
    })
}

/// Reject a suggested Transfer, or unlink a confirmed one: its
/// Transactions count as Income or Expenses again, and the row is kept as
/// `rejected`, so the same pair is never suggested again.
///
/// A Rule's Transfer with one side that was later completed goes back to
/// having only that side: the Rule still says it is a Transfer, only the
/// pairing was wrong.
pub fn reject(store: &mut Store, transfer_id: &str) -> Result<Transfer> {
    store.write(|w| {
        let transfer = Transfer {
            status: LinkStatus::Rejected,
            ..find_live(w.conn(), transfer_id)?
        };
        w.upsert(&transfer)?;
        if transfer.origin == TransferOrigin::Rule {
            for txn_id in [&transfer.out_txn_id, &transfer.in_txn_id]
                .into_iter()
                .flatten()
            {
                let alone = transfers_with_deleted(w.conn(), &side_id(txn_id))?;
                if let Some(alone) = alone.filter(|t| t.origin == TransferOrigin::Rule) {
                    w.upsert(&Transfer {
                        status: LinkStatus::Confirmed,
                        ..alone
                    })?;
                }
            }
        }
        Ok(transfer)
    })
}

fn find_live(conn: &Connection, id: &str) -> Result<Transfer> {
    transfers_where(conn, "id = ?1", [id])?
        .pop()
        .ok_or_else(|| Error::NotFound {
            entity: Transfer::ENTITY,
            id: id.to_owned(),
        })
}

/// The Transfer row `id`, even if it was deleted.
fn transfers_with_deleted(conn: &Connection, id: &str) -> Result<Option<Transfer>> {
    Ok(transfers_matching(conn, "id = ?1", [id])?.pop())
}

/// The Transfer (not rejected) that `txn_id` is a side of, if any.
fn live_transfer_of(conn: &Connection, txn_id: &str) -> Result<Option<Transfer>> {
    Ok(transfers_where(
        conn,
        "status <> 'rejected' AND (out_txn_id = ?1 OR in_txn_id = ?1)",
        params![txn_id],
    )?
    .pop())
}

/// The live Transfer rows matching `filter`.
fn transfers_where(
    conn: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<Transfer>> {
    transfers_matching(conn, &format!("deleted = 0 AND {filter}"), args)
}

/// Every Transfer row matching `filter`, deleted ones included.
fn transfers_matching(
    conn: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<Transfer>> {
    let sql = format!(
        "SELECT id, out_txn_id, in_txn_id, status, origin FROM transfer
         WHERE {filter} ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(args, |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, out_txn_id, in_txn_id, status, origin) = row?;
        Ok(Transfer {
            id,
            out_txn_id,
            in_txn_id,
            status: LinkStatus::parse(&status)?,
            origin: TransferOrigin::parse(&origin)?,
        })
    })
    .collect()
}

/// A Transfer as shown in the app, with its live Transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransferView {
    #[serde(flatten)]
    pub transfer: Transfer,
    /// `None` while the outgoing side hasn't been imported (or was deleted).
    pub out_txn: Option<TransactionView>,
    /// `None` while the incoming side hasn't been imported (or was deleted).
    pub in_txn: Option<TransactionView>,
}

impl TransferView {
    /// Whether both sides are there.
    pub fn is_complete(&self) -> bool {
        self.out_txn.is_some() && self.in_txn.is_some()
    }

    /// The date of its earliest side.
    pub fn date(&self) -> Option<Date> {
        [&self.out_txn, &self.in_txn]
            .into_iter()
            .flatten()
            .filter_map(|t| t.transaction.date.parse().ok())
            .min()
    }
}

/// The Savings target next to what Transfers actually moved into savings
/// Accounts in a Period (sync-design §1.7 "Savings progress").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavingsProgress {
    pub period: Period,
    /// The Savings target per Period (0 when not set).
    pub target_minor: i64,
    /// The incoming sides on savings Accounts dated in the Period, of
    /// Transfers that aren't rejected and don't come from another savings
    /// Account.
    pub transferred_minor: i64,
}

/// Savings progress for `period`.
pub fn savings_progress(store: &Store, period: Period) -> Result<SavingsProgress> {
    let transferred_minor = store.conn().query_row(
        "SELECT COALESCE(SUM(i.amount_minor), 0)
         FROM transfer tr
         JOIN txn i ON i.id = tr.in_txn_id AND i.deleted = 0
         JOIN account ia ON ia.id = i.account_id AND ia.deleted = 0 AND ia.kind = 'savings'
         LEFT JOIN txn o ON o.id = tr.out_txn_id AND o.deleted = 0
         LEFT JOIN account oa ON oa.id = o.account_id AND oa.deleted = 0
         WHERE tr.deleted = 0 AND tr.status <> 'rejected'
           AND i.date BETWEEN ?1 AND ?2
           AND (oa.kind IS NULL OR oa.kind <> 'savings')",
        params![
            period.first_day().to_string(),
            period.last_day().to_string()
        ],
        |r| r.get(0),
    )?;
    Ok(SavingsProgress {
        period,
        target_minor: settings::savings_target_minor(store)?,
        transferred_minor,
    })
}

/// Every Transfer that isn't rejected and has at least one live side,
/// oldest first.
pub fn list(store: &Store) -> Result<Vec<TransferView>> {
    let conn = store.conn();
    let mut views = Vec::new();
    for transfer in transfers_where(conn, "status <> 'rejected'", [])? {
        let side = |id: &Option<String>| -> Result<Option<TransactionView>> {
            id.as_deref()
                .map(|id| transactions::view_live(conn, id))
                .transpose()
                .map(Option::flatten)
        };
        let out_txn = side(&transfer.out_txn_id)?;
        let in_txn = side(&transfer.in_txn_id)?;
        if out_txn.is_none() && in_txn.is_none() {
            continue;
        }
        views.push(TransferView {
            transfer,
            out_txn,
            in_txn,
        });
    }
    views.sort_by_key(|v| (v.date(), v.transfer.id.clone()));
    Ok(views)
}
