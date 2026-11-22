//! The review list: the Suggestions waiting for the user to confirm or
//! reject (CONTEXT.md "Suggestion"), and Transfers whose other side still
//! hasn't been imported after their Period ended.
//!
//! Every Suggestion is a row with a [`LinkStatus`] (sync-design §1.1). Until
//! it is decided, the numbers count a `suggested` link as if it had been
//! confirmed. A `rejected` row is kept, so the same pair is never suggested
//! again.
//!
//! Ticket 06 adds the Transfer items, ticket 07 the Manual Transaction
//! matches ([`crate::manual_matches`]). Ticket 09 (Planned item fulfilment)
//! adds its own [`ReviewItem`] variant and fills it in [`list`].

use serde::{Deserialize, Serialize};

use crate::date::Date;
use crate::error::{Error, Result};
use crate::manual_matches::{self, ManualMatchView};
use crate::period::Period;
use crate::store::Store;
use crate::transfers::{self, TransferView};

/// Where a link (a Suggestion) stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    /// Proposed by the app, not decided yet. Counted as if confirmed.
    Suggested,
    Confirmed,
    /// Kept so the same pair is never suggested again. Not counted.
    Rejected,
}

impl LinkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkStatus::Suggested => "suggested",
            LinkStatus::Confirmed => "confirmed",
            LinkStatus::Rejected => "rejected",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "suggested" => Ok(LinkStatus::Suggested),
            "confirmed" => Ok(LinkStatus::Confirmed),
            "rejected" => Ok(LinkStatus::Rejected),
            other => Err(Error::Codec(format!("unknown link status {other:?}"))),
        }
    }
}

/// One entry of the review list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReviewItem {
    /// Two Transactions the app thinks form a Transfer: confirm or reject.
    TransferSuggestion { transfer: TransferView },
    /// A Transfer with only one side whose Period has ended: "other side
    /// not imported yet". It still counts as a Transfer; nothing to decide.
    TransferMissingSide { transfer: TransferView },
    /// An imported Transaction that looks like a Manual Transaction the
    /// user entered by hand: confirm (merge them) or reject.
    ManualMatchSuggestion { manual_match: ManualMatchView },
}

impl ReviewItem {
    /// Whether the user has to decide something (counted in the badge).
    pub fn is_pending(&self) -> bool {
        matches!(
            self,
            ReviewItem::TransferSuggestion { .. } | ReviewItem::ManualMatchSuggestion { .. }
        )
    }
}

/// The review list, with the count for the "to review" badge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewList {
    /// Pending Suggestions first (oldest first), then the informational items.
    pub items: Vec<ReviewItem>,
    /// How many items wait for a decision.
    pub pending_count: usize,
}

/// Everything to review as of `today`.
pub fn list(store: &Store, today: Date) -> Result<ReviewList> {
    let this_period = Period::containing(today);
    let mut pending = Vec::new();
    let mut missing = Vec::new();
    for transfer in transfers::list(store)? {
        if transfer.transfer.status == LinkStatus::Suggested && transfer.is_complete() {
            pending.push(ReviewItem::TransferSuggestion { transfer });
        } else if !transfer.is_complete()
            && transfer
                .date()
                .is_some_and(|d| Period::containing(d) < this_period)
        {
            missing.push(ReviewItem::TransferMissingSide { transfer });
        }
    }
    for manual_match in manual_matches::suggestions(store)? {
        pending.push(ReviewItem::ManualMatchSuggestion { manual_match });
    }
    let pending_count = pending.len();
    pending.extend(missing);
    Ok(ReviewList {
        items: pending,
        pending_count,
    })
}
