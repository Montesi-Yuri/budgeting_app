//! Spendable and Weekly Spendable (CONTEXT.md, sync-design §1.7):
//! calculated, never stored. "Today" is always passed in.
//!
//! Spendable for a Period, all values signed with money out negative:
//!
//! ```text
//!   Income in the Period
//! + Expenses in the Period (Refunds included, as positive amounts)
//! + Uncategorised Transactions in the Period (treated as Expenses/Refunds)
//! + expected Income occurrences of Planned items in the Period
//! + expected Expense occurrences of Planned items in the Period
//! + Set-asides for the Period (negative for Expenses)
//! − Savings target
//! ```
//!
//! Each Period starts fresh: only the Period's own Transactions and expected
//! occurrences count. A Planned item spread as Set-asides counts only as its
//! Set-aside: its own occurrences are left out, so the Period it falls due in
//! isn't hit twice (see [`crate::planned`]).
//!
//! Later tickets extend this: Transfers are left out of the totals (06),
//! Manual Transactions hidden by a match too (07); fulfilled occurrences stop
//! being expected, and Transactions fulfilling an item spread as Set-asides
//! are left out of the totals (09). A new term is a new field on
//! [`PeriodSpendable`], filled in [`for_period`] and added in
//! [`PeriodSpendable::calculate`].

use serde::Serialize;

use crate::date::Date;
use crate::error::Result;
use crate::period::{Period, Week};
use crate::planned::{self, PeriodPlan};
use crate::settings;
use crate::store::Store;
use crate::totals::{self, Totals};

/// Spendable for one Period, with the terms it is made of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeriodSpendable {
    pub period: Period,
    pub first_day: Date,
    pub last_day: Date,
    /// Income in the Period.
    pub income_minor: i64,
    /// Expenses in the Period, negative, reduced by Refunds.
    pub expenses_minor: i64,
    /// Uncategorised Transactions in the Period: they can't be Income, so
    /// a negative one counts as an Expense and a positive one as a Refund.
    pub uncategorised_minor: i64,
    /// Expected Income occurrences of Planned items in the Period that are
    /// not spread as Set-asides.
    pub expected_income_minor: i64,
    /// Expected Expense occurrences of Planned items in the Period that are
    /// not spread as Set-asides (negative).
    pub expected_expenses_minor: i64,
    /// The Period's Set-asides, signed: negative for Expenses.
    pub set_asides_minor: i64,
    /// The Savings target, set aside from every Period (positive).
    pub savings_target_minor: i64,
    /// How much the user can still spend in the Period. Negative when
    /// overspent.
    pub spendable_minor: i64,
    /// The Weekly Spendable for today's Week. `None` unless today is in
    /// this Period: past Periods are over, future ones haven't started.
    pub weekly: Option<WeeklySpendable>,
}

impl PeriodSpendable {
    /// Pure calculation from the Period's totals, its Planned item
    /// occurrences and Set-asides, and the Savings target.
    pub fn calculate(
        period: Period,
        totals: &Totals,
        plan: &PeriodPlan,
        savings_target_minor: i64,
        today: Date,
    ) -> PeriodSpendable {
        let expected_income_minor = plan.expected_income_minor();
        let expected_expenses_minor = plan.expected_expenses_minor();
        let set_asides_minor = plan.set_asides_minor();
        let spendable_minor = totals.income_minor
            + totals.expenses_minor
            + totals.uncategorised_minor
            + expected_income_minor
            + expected_expenses_minor
            + set_asides_minor
            - savings_target_minor;
        PeriodSpendable {
            period,
            first_day: period.first_day(),
            last_day: period.last_day(),
            income_minor: totals.income_minor,
            expenses_minor: totals.expenses_minor,
            uncategorised_minor: totals.uncategorised_minor,
            expected_income_minor,
            expected_expenses_minor,
            set_asides_minor,
            savings_target_minor,
            spendable_minor,
            weekly: (Period::containing(today) == period)
                .then(|| weekly_spendable(spendable_minor, today)),
        }
    }
}

/// Spendable for `period` from the stored Transactions, Planned items and
/// settings, with the Weekly Spendable when `today` falls in it.
pub fn for_period(store: &Store, period: Period, today: Date) -> Result<PeriodSpendable> {
    let totals = totals::for_dates(
        store,
        &period.first_day().to_string(),
        &period.last_day().to_string(),
    )?;
    let plan = planned::for_period(store, period)?;
    let savings_target = settings::savings_target_minor(store)?;
    Ok(PeriodSpendable::calculate(
        period,
        &totals,
        &plan,
        savings_target,
        today,
    ))
}

/// The Weekly Spendable for the Week containing `today`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeeklySpendable {
    /// The Week, cut at the Period boundary.
    pub week_first_day: Date,
    pub week_last_day: Date,
    /// Counting today.
    pub days_left_in_period: u32,
    /// Counting today.
    pub days_left_in_week: u32,
    pub amount_minor: i64,
}

/// The Period's remaining Spendable, divided by the days left in the Period
/// and multiplied by the days left in the current Week (both counting
/// today). Rounded down, so it never offers a fraction of a minor unit more
/// than is there. Recalculated daily, it makes overspending shrink the
/// following Weeks and underspending grow them.
pub fn weekly_spendable(remaining_minor: i64, today: Date) -> WeeklySpendable {
    let week = Week::containing(today);
    let days_left_in_period = Period::containing(today).days_left(today);
    let days_left_in_week = week.days_left(today);
    let amount = (i128::from(remaining_minor) * i128::from(days_left_in_week))
        .div_euclid(i128::from(days_left_in_period));
    WeeklySpendable {
        week_first_day: week.first_day(),
        week_last_day: week.last_day(),
        days_left_in_period,
        days_left_in_week,
        amount_minor: amount as i64,
    }
}
