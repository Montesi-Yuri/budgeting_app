//! Totals of Income and Expenses over a date range (calculated, never stored).
//!
//! Whether a Transaction is Income or an Expense is decided by the kind of
//! its Category, not by the sign of its amount. A positive Transaction in an
//! expense Category is a Refund: it reduces that Category's spending and is
//! not Income. Transactions without a (live) Category are Uncategorised.
//!
//! Transactions in a Transfer that isn't rejected are left out: a Transfer
//! is never Income or an Expense (see [`crate::transfers`]). So are Manual
//! Transactions hidden by a match: only the imported Transaction counts
//! (see [`crate::manual_matches`]).

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::categories::CategoryKind;
use crate::error::Result;
use crate::store::Store;
use crate::{manual_matches, transfers};

/// The signed sum of one Category's Transactions in the range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTotal {
    pub category_id: String,
    pub kind: CategoryKind,
    /// Signed sum of the amounts: spending is negative, Refunds positive.
    pub net_minor: i64,
}

/// Signed totals (money out is negative) for a date range.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Totals {
    /// Sum of the Transactions in income Categories.
    pub income_minor: i64,
    /// Sum of the Transactions in expense Categories, Refunds included.
    pub expenses_minor: i64,
    /// Sum of the Uncategorised Transactions.
    pub uncategorised_minor: i64,
    /// Per Category that has Transactions in the range (children are not
    /// rolled up into their parents).
    pub categories: Vec<CategoryTotal>,
}

impl Totals {
    /// How much was spent in an expense Category: positive when money went
    /// out, reduced by Refunds. Always 0 for an income Category.
    pub fn spending_minor(&self, category_id: &str) -> i64 {
        self.categories
            .iter()
            .find(|c| c.category_id == category_id && c.kind == CategoryKind::Expense)
            .map_or(0, |c| -c.net_minor)
    }
}

/// Totals of the live Transactions on live Accounts dated `from..=to`
/// (`YYYY-MM-DD`), leaving out those in a Transfer that isn't rejected and
/// the Manual Transactions hidden by a match.
pub fn for_dates(store: &Store, from: &str, to: &str) -> Result<Totals> {
    let sql = format!(
        "SELECT c.id, c.kind, SUM(t.amount_minor)
         FROM txn t
         JOIN account a ON a.id = t.account_id AND a.deleted = 0
         LEFT JOIN category c ON c.id = t.category_id AND c.deleted = 0
         WHERE t.deleted = 0 AND t.date BETWEEN ?1 AND ?2
           AND t.id NOT IN ({})
           AND t.id NOT IN ({})
         GROUP BY c.id, c.kind
         ORDER BY c.id",
        transfers::IN_TRANSFER_SQL,
        manual_matches::HIDDEN_MANUAL_SQL
    );
    let mut stmt = store.conn().prepare(&sql)?;
    let rows = stmt.query_map(params![from, to], |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut totals = Totals::default();
    for row in rows {
        let (category_id, kind, net_minor) = row?;
        let (Some(category_id), Some(kind)) = (category_id, kind) else {
            totals.uncategorised_minor += net_minor;
            continue;
        };
        let kind = CategoryKind::parse(&kind)?;
        match kind {
            CategoryKind::Income => totals.income_minor += net_minor,
            CategoryKind::Expense => totals.expenses_minor += net_minor,
        }
        totals.categories.push(CategoryTotal {
            category_id,
            kind,
            net_minor,
        });
    }
    Ok(totals)
}
