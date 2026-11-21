//! Budgets: an optional spending limit for one expense Category, per Period
//! or per Week (CONTEXT.md). A Budget on a parent Category covers its
//! children. A Budget never reserves money: Spendable covers all spending,
//! and nothing here is read by Spendable. A Budget only shows progress and
//! warns when its Category goes over.
//!
//! Spending comes from [`totals::for_dates`], so Refunds reduce it and
//! whatever `totals` leaves out (Transfers, once ticket 06 lands) is left out
//! here too.
//!
//! This module also has small Period and Week helpers ([`period_of`],
//! [`week_of`]); "today" is always passed in.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::categories::{self, CategoryKind};
use crate::date;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow};
use crate::totals;

/// Whether a Budget's limit is per Period or per Week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPer {
    Period,
    Week,
}

impl BudgetPer {
    fn as_str(self) -> &'static str {
        match self {
            BudgetPer::Period => "period",
            BudgetPer::Week => "week",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "period" => Ok(BudgetPer::Period),
            "week" => Ok(BudgetPer::Week),
            other => Err(Error::Codec(format!("unknown Budget per {other:?}"))),
        }
    }
}

/// One Budget row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    pub id: String,
    pub category_id: String,
    pub per: BudgetPer,
    /// The limit per Period or Week, in minor units; always positive.
    pub amount_minor: i64,
    /// The first day the Budget applies.
    pub valid_from: String,
    /// The last day the Budget applies; `None` until it is ended.
    pub valid_to: Option<String>,
}

impl SyncedRow for Budget {
    const ENTITY: &'static str = "budget";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO budget (id, category_id, per, amount_minor, valid_from, valid_to, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
             ON CONFLICT(id) DO UPDATE SET
               category_id = excluded.category_id, per = excluded.per,
               amount_minor = excluded.amount_minor, valid_from = excluded.valid_from,
               valid_to = excluded.valid_to, updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.category_id,
                self.per.as_str(),
                self.amount_minor,
                self.valid_from,
                self.valid_to,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// What the user enters to create a Budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBudget {
    pub category_id: String,
    pub per: BudgetPer,
    pub amount_minor: i64,
    pub valid_from: String,
}

/// Create a Budget on an expense Category.
pub fn create(store: &mut Store, new: NewBudget) -> Result<Budget> {
    store.write(|w| {
        let budget = Budget {
            id: uuid::Uuid::now_v7().to_string(),
            category_id: new.category_id,
            per: new.per,
            amount_minor: new.amount_minor,
            valid_from: new.valid_from,
            valid_to: None,
        };
        validate(w.conn(), &budget)?;
        w.upsert(&budget)?;
        Ok(budget)
    })
}

/// What the user can change on a Budget. Its end date stays as it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetEdit {
    pub id: String,
    pub category_id: String,
    pub per: BudgetPer,
    pub amount_minor: i64,
    pub valid_from: String,
}

/// Change a Budget's Category, amount, per or start date. Choosing a live
/// Category again repairs a broken Budget.
pub fn update(store: &mut Store, edit: BudgetEdit) -> Result<Budget> {
    store.write(|w| {
        let budget = Budget {
            category_id: edit.category_id,
            per: edit.per,
            amount_minor: edit.amount_minor,
            valid_from: edit.valid_from,
            ..find_live(w.conn(), &edit.id)?
        };
        validate(w.conn(), &budget)?;
        w.upsert(&budget)?;
        Ok(budget)
    })
}

/// End a Budget: `last_day` is the last day it applies.
pub fn end(store: &mut Store, id: &str, last_day: &str) -> Result<Budget> {
    store.write(|w| {
        let budget = Budget {
            valid_to: Some(last_day.to_owned()),
            ..find_live(w.conn(), id)?
        };
        if !date::is_valid(last_day) || last_day < budget.valid_from.as_str() {
            return Err(Error::invalid(
                "A Budget can't end before the date it starts applying.",
            ));
        }
        w.upsert(&budget)?;
        Ok(budget)
    })
}

/// Delete a Budget (a soft delete), e.g. one entered by mistake. To stop a
/// Budget but keep its history, [`end`] it instead.
pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.write(|w| {
        w.delete::<Budget>(id)?;
        Ok(())
    })
}

fn find_live(conn: &Connection, id: &str) -> Result<Budget> {
    live_budgets(conn)?
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| Error::NotFound {
            entity: Budget::ENTITY,
            id: id.to_owned(),
        })
}

/// The rules for a Budget the user saves.
fn validate(conn: &Connection, budget: &Budget) -> Result<()> {
    let category = categories::live_categories(conn)?
        .into_iter()
        .find(|c| c.id == budget.category_id)
        .ok_or_else(|| Error::invalid("The Budget's Category doesn't exist."))?;
    if category.kind != CategoryKind::Expense {
        return Err(Error::invalid(
            "A Budget limits spending, so it needs an expense Category.",
        ));
    }
    if budget.amount_minor <= 0 {
        return Err(Error::invalid("A Budget's amount must be more than zero."));
    }
    if !date::is_valid(&budget.valid_from) {
        return Err(Error::invalid(
            "Enter the date the Budget starts applying (YYYY-MM-DD).",
        ));
    }
    if let Some(valid_to) = &budget.valid_to {
        if !date::is_valid(valid_to) || valid_to.as_str() < budget.valid_from.as_str() {
            return Err(Error::invalid(
                "A Budget can't end before the date it starts applying.",
            ));
        }
    }
    Ok(())
}

/// Every live Budget, oldest first.
pub fn list(store: &Store) -> Result<Vec<Budget>> {
    live_budgets(store.conn())
}

fn live_budgets(conn: &Connection) -> Result<Vec<Budget>> {
    let mut stmt = conn.prepare(
        "SELECT id, category_id, per, amount_minor, valid_from, valid_to FROM budget
         WHERE deleted = 0 ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    rows.map(|row| {
        let (id, category_id, per, amount_minor, valid_from, valid_to) = row?;
        Ok(Budget {
            id,
            category_id,
            per: BudgetPer::parse(&per)?,
            amount_minor,
            valid_from,
            valid_to,
        })
    })
    .collect()
}

/// Whether a Budget applies on the day asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetState {
    /// It applies today; its progress is counted.
    Active,
    /// It starts applying after today.
    Upcoming,
    /// Its last day was before today.
    Ended,
    /// Its Category was deleted (perhaps on another Device). The Budget
    /// stays, but is not applied until the user picks a live Category
    /// (sync-design §6).
    Broken,
}

/// A Budget with its progress in the current Period or Week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetProgress {
    pub budget: Budget,
    pub state: BudgetState,
    /// The days counted: the current Period or Week. `None` unless the
    /// Budget is [`BudgetState::Active`].
    pub range: Option<DateRange>,
    /// Spending in `range`, reduced by Refunds (0 when there's no range).
    pub spent_minor: i64,
    /// `amount_minor - spent_minor`; negative when over.
    pub left_minor: i64,
    /// Spent more than the limit.
    pub over: bool,
}

/// Every live Budget with its progress on `today` (`YYYY-MM-DD`).
pub fn progress(store: &Store, today: &str) -> Result<Vec<BudgetProgress>> {
    let all_categories = categories::list(store)?;
    let period = period_of(today)?;
    let week = week_of(today)?;
    live_budgets(store.conn())?
        .into_iter()
        .map(|budget| {
            let category_is_live = all_categories
                .iter()
                .any(|c| c.id == budget.category_id && c.kind == CategoryKind::Expense);
            let state = if !category_is_live {
                BudgetState::Broken
            } else if today < budget.valid_from.as_str() {
                BudgetState::Upcoming
            } else if budget.valid_to.as_deref().is_some_and(|to| to < today) {
                BudgetState::Ended
            } else {
                BudgetState::Active
            };
            let range = (state == BudgetState::Active).then(|| match budget.per {
                BudgetPer::Period => period.clone(),
                BudgetPer::Week => week.clone(),
            });
            let spent_minor = match &range {
                None => 0,
                Some(range) => {
                    let totals = totals::for_dates(store, &range.from, &range.to)?;
                    // The Category itself and, for a parent, its children.
                    all_categories
                        .iter()
                        .filter(|c| {
                            c.id == budget.category_id
                                || c.parent_id.as_deref() == Some(budget.category_id.as_str())
                        })
                        .map(|c| totals.spending_minor(&c.id))
                        .sum()
                }
            };
            Ok(BudgetProgress {
                left_minor: budget.amount_minor - spent_minor,
                over: spent_minor > budget.amount_minor,
                spent_minor,
                range,
                state,
                budget,
            })
        })
        .collect()
}

/// An inclusive range of `YYYY-MM-DD` dates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

/// The Period (calendar month) that contains `day`.
pub fn period_of(day: &str) -> Result<DateRange> {
    let (y, m, _) = parse(day)?;
    Ok(DateRange {
        from: format_ymd(y, m, 1),
        to: format_ymd(y, m, days_in_month(y, m)),
    })
}

/// The Week (Monday to Sunday, cut at the Period boundary) that contains `day`.
pub fn week_of(day: &str) -> Result<DateRange> {
    let (y, m, d) = parse(day)?;
    // 0 = Monday ... 6 = Sunday
    let weekday = (days_from_civil(y, m, d) + 3).rem_euclid(7) as u32;
    let monday = d as i64 - weekday as i64;
    let sunday = monday + 6;
    Ok(DateRange {
        from: format_ymd(y, m, monday.max(1) as u32),
        to: format_ymd(y, m, sunday.min(days_in_month(y, m) as i64) as u32),
    })
}

fn parse(day: &str) -> Result<(i32, u32, u32)> {
    if !date::is_valid(day) {
        return Err(Error::invalid(format!(
            "{day:?} is not a date (YYYY-MM-DD)."
        )));
    }
    let num = |r: std::ops::Range<usize>| day[r].parse::<u32>().unwrap_or(0);
    Ok((num(0..4) as i32, num(5..7), num(8..10)))
}

fn format_ymd(y: i32, m: u32, d: u32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_in_month(y: i32, m: u32) -> u32 {
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    match m {
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 31,
    }
}

/// Days since 1970-01-01 (a Thursday), after Howard Hinnant's algorithm.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
