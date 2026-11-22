//! The Forecast (CONTEXT.md): the projected balance at the end of the
//! current Period and at the end of the year, for the Net position (the
//! main number) and for the main Account. Calculated, never stored
//! (sync-design §1.7); "today" is always passed in.
//!
//! The projection starts from today's Account balances and, Period by
//! Period until December, adds:
//!
//! - the expected occurrences of Planned items due after today, on their
//!   own Account and in the month they fall due (an item spread as
//!   Set-asides still hits its actual month: Set-asides only shape
//!   Spendable, they don't move money);
//! - estimated everyday spending per expense Category ([`EverydayEstimate`]).
//!
//! Everyday spending, per Period:
//!
//! - an expense Category with a Budget that applies in the Period uses the
//!   Budget (a Budget per Week counts days in Period / 7 Weeks); a Budget
//!   on a parent covers its children;
//! - any other expense Category uses its average spending (Refunds
//!   included) over the last 3 complete Periods, or over the complete
//!   Periods there are; Uncategorised spending counts too (CONTEXT.md);
//! - a Category with a Planned item that hasn't ended is left out: its
//!   spending is already expected from the Planned item;
//! - in the current Period only what is left counts: the estimate less what
//!   was already spent this Period, never below 0.
//!
//! Everyday spending is taken from the main Account (credit card spending
//! ends up there when the card is paid off); for the Net position it makes
//! no difference which Account it leaves from.
//!
//! A complete Period of history is one that ended before the current
//! Period and started on or after the history's first day: the day after
//! the earliest Opening balance, or the earliest Transaction if that is
//! earlier. With fewer than 3, the Forecast is low confidence.
//!
//! Transfers are never projected, and they already sit in both Accounts'
//! balances, so they move money between Accounts without changing the Net
//! position. They are left out of the history by [`totals::for_dates`].

use serde::Serialize;

use crate::accounts;
use crate::budgets::{self, Budget, BudgetPer};
use crate::categories::{self, Category, CategoryKind};
use crate::date::{days_in_month, Date};
use crate::error::Result;
use crate::period::Period;
use crate::planned::{self, Occurrence};
use crate::store::Store;
use crate::totals::{self, Totals};

/// The projected balances on one day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForecastPoint {
    pub date: Date,
    /// All Accounts, credit card debt negative.
    pub net_position_minor: i64,
    /// The main Account; `None` when no Account is marked as main.
    pub main_account_minor: Option<i64>,
}

/// Where an everyday spending estimate comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EverydaySource {
    /// The Category's Budget (a Budget on a parent covers its children).
    Budget,
    /// The average of the last 3 complete Periods.
    Average,
}

/// The everyday spending expected in one expense Category per Period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EverydayEstimate {
    pub category_id: Option<String>,
    pub name: String,
    pub source: EverydaySource,
    /// Spending expected in the current Period (positive).
    pub per_period_minor: i64,
    /// What is still expected this Period: the estimate less what was
    /// already spent, never below 0.
    pub left_this_period_minor: i64,
}

/// One Period of the projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForecastMonth {
    pub period: Period,
    /// Expected Planned item occurrences in the Period after today, signed.
    pub planned_minor: i64,
    /// Estimated everyday spending in the Period after today (positive).
    pub everyday_minor: i64,
    /// The projected Net position at the end of the Period.
    pub net_position_minor: i64,
    /// The projected main Account balance at the end of the Period.
    pub main_account_minor: Option<i64>,
}

/// The Forecast on `today`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Forecast {
    /// Today's balances, where the projection starts.
    pub today: ForecastPoint,
    pub end_of_period: ForecastPoint,
    pub end_of_year: ForecastPoint,
    /// The name of the main Account, if one is marked as main.
    pub main_account_name: Option<String>,
    /// From the current Period to December, month by month.
    pub months: Vec<ForecastMonth>,
    /// The current Period's everyday spending estimates.
    pub everyday: Vec<EverydayEstimate>,
    /// Complete Periods of history before the current one.
    pub complete_periods: u32,
    /// Fewer than 3 complete Periods of history.
    pub low_confidence: bool,
}

/// The Forecast for `today`, from the stored data.
pub fn for_today(store: &Store, today: Date) -> Result<Forecast> {
    let accounts = accounts::list(store)?;
    let period = Period::containing(today);
    let main = accounts.iter().find(|a| a.account.is_main);
    let year_end = Date::new(today.year(), 12, 31).expect("31 December is a date");
    let complete_periods = complete_periods(history_start(store, &accounts)?, period);
    let mut history = Vec::new();
    let mut past = period;
    for _ in 0..complete_periods.min(HISTORY_PERIODS) {
        past = past.previous();
        history.push(totals::for_dates(
            store,
            &past.first_day().to_string(),
            &past.last_day().to_string(),
        )?);
    }
    let data = Data {
        today,
        net_position_minor: accounts.iter().map(|a| a.balance_minor).sum(),
        main_account_id: main.map(|a| a.account.id.clone()),
        main_account_name: main.map(|a| a.account.name.clone()),
        main_account_minor: main.map(|a| a.balance_minor),
        occurrences: planned::expected_occurrences(store, today.add_days(1), year_end)?,
        planned_categories: planned::list(store)?
            .into_iter()
            .filter(|i| i.end_date.is_none_or(|end| end >= today))
            .map(|i| i.category_id)
            .collect(),
        categories: categories::list(store)?,
        budgets: budgets::list(store)?,
        this_period: totals::for_dates(store, &period.first_day().to_string(), &today.to_string())?,
        complete_periods,
        history,
    };
    Ok(calculate(&data))
}

/// How many complete Periods everyday spending is averaged over.
pub const HISTORY_PERIODS: u32 = 3;

/// The first day whose Transactions are all known: the day after the
/// earliest Opening balance (balances are calculated forward from it), or
/// the earliest Transaction if there is one before that. `None` without
/// Accounts.
fn history_start(store: &Store, accounts: &[accounts::AccountWithBalance]) -> Result<Option<Date>> {
    let mut start: Option<Date> = None;
    for a in accounts {
        let day = a.account.opening_date.parse::<Date>()?.add_days(1);
        start = Some(start.map_or(day, |s| s.min(day)));
    }
    let earliest: Option<String> = store.conn().query_row(
        "SELECT MIN(t.date) FROM txn t
         JOIN account a ON a.id = t.account_id AND a.deleted = 0
         WHERE t.deleted = 0",
        [],
        |r| r.get(0),
    )?;
    if let Some(earliest) = earliest {
        let day = earliest.parse::<Date>()?;
        start = Some(start.map_or(day, |s| s.min(day)));
    }
    Ok(start)
}

/// The Periods before `current` that history covers from their first day.
fn complete_periods(history_start: Option<Date>, current: Period) -> u32 {
    let Some(start) = history_start else {
        return 0;
    };
    let mut count = 0;
    let mut period = current.previous();
    while period.first_day() >= start {
        count += 1;
        period = period.previous();
    }
    count
}

/// Everything the Forecast is calculated from.
struct Data {
    today: Date,
    net_position_minor: i64,
    main_account_id: Option<String>,
    main_account_name: Option<String>,
    main_account_minor: Option<i64>,
    /// Expected Planned item occurrences after today, to the end of the year.
    occurrences: Vec<Occurrence>,
    /// The Categories of Planned items that haven't ended: their spending
    /// is already expected, so it isn't estimated again.
    planned_categories: Vec<String>,
    categories: Vec<Category>,
    budgets: Vec<Budget>,
    /// Totals of the current Period up to today.
    this_period: Totals,
    complete_periods: u32,
    /// Totals of the last (up to 3) complete Periods, latest first.
    history: Vec<Totals>,
}

/// One line of everyday spending: a set of expense Categories estimated
/// together.
struct Line {
    /// `None` for the Uncategorised Transactions.
    category_id: Option<String>,
    name: String,
    /// The Category and, for a parent with a Budget, its children.
    covers: Vec<String>,
    source: EverydaySource,
}

fn calculate(data: &Data) -> Forecast {
    let current = Period::containing(data.today);
    let mut net = data.net_position_minor;
    let mut main = data.main_account_minor;
    let today = point(data.today, net, main);
    let mut everyday = Vec::new();
    let mut end_of_period = today.clone();
    let mut months = Vec::new();
    let mut period = current;
    loop {
        let (mut planned_minor, mut planned_main) = (0, 0);
        for o in data
            .occurrences
            .iter()
            .filter(|o| Period::containing(o.due_date) == period)
        {
            planned_minor += o.amount_minor;
            if data.main_account_id.as_deref() == Some(o.account_id.as_str()) {
                planned_main += o.amount_minor;
            }
        }
        let mut everyday_minor = 0;
        for line in lines(data, period) {
            let per_period = line_amount(data, &line, period);
            let amount = if period == current {
                let spent = spending(&data.this_period, &line);
                let left = (per_period - spent).max(0);
                everyday.push(EverydayEstimate {
                    category_id: line.category_id.clone(),
                    name: line.name.clone(),
                    source: line.source,
                    per_period_minor: per_period,
                    left_this_period_minor: left,
                });
                left
            } else {
                per_period
            };
            everyday_minor += amount;
        }
        net += planned_minor - everyday_minor;
        main = main.map(|m| m + planned_main - everyday_minor);
        if period == current {
            end_of_period = point(period.last_day(), net, main);
        }
        months.push(ForecastMonth {
            period,
            planned_minor,
            everyday_minor,
            net_position_minor: net,
            main_account_minor: main,
        });
        if period.last_day().month() == 12 {
            break;
        }
        period = period.next();
    }
    Forecast {
        today,
        end_of_period,
        end_of_year: point(period.last_day(), net, main),
        main_account_name: data.main_account_name.clone(),
        months,
        everyday,
        complete_periods: data.complete_periods,
        low_confidence: data.complete_periods < HISTORY_PERIODS,
    }
}

fn point(date: Date, net_position_minor: i64, main_account_minor: Option<i64>) -> ForecastPoint {
    ForecastPoint {
        date,
        net_position_minor,
        main_account_minor,
    }
}

/// The everyday spending lines for `period`. A Budget on a parent covers
/// its children; every other expense Category with spending in its
/// history is estimated from its average.
fn lines(data: &Data, period: Period) -> Vec<Line> {
    let expense: Vec<&Category> = data
        .categories
        .iter()
        .filter(|c| c.kind == CategoryKind::Expense)
        .collect();
    let mut out = Vec::new();
    let planned = |c: &Category| data.planned_categories.contains(&c.id);
    for parent in expense.iter().filter(|c| c.parent_id.is_none()) {
        let children: Vec<&Category> = expense
            .iter()
            .copied()
            .filter(|c| c.parent_id.as_deref() == Some(parent.id.as_str()))
            .collect();
        if !planned(parent) && budget_for(data, &parent.id, period).is_some() {
            let mut covers = vec![parent.id.clone()];
            covers.extend(children.iter().map(|c| c.id.clone()));
            out.push(Line {
                category_id: Some(parent.id.clone()),
                name: parent.name.clone(),
                covers,
                source: EverydaySource::Budget,
            });
            continue;
        }
        for category in std::iter::once(*parent).chain(children) {
            if planned(category) {
                continue;
            }
            let source = if budget_for(data, &category.id, period).is_some() {
                EverydaySource::Budget
            } else {
                EverydaySource::Average
            };
            let line = Line {
                category_id: Some(category.id.clone()),
                name: category.name.clone(),
                covers: vec![category.id.clone()],
                source,
            };
            if source == EverydaySource::Budget || average(data, &line) > 0 {
                out.push(line);
            }
        }
    }
    // Uncategorised Transactions still count (CONTEXT.md); they can't have
    // a Budget.
    let uncategorised = Line {
        category_id: None,
        name: "Uncategorised".into(),
        covers: Vec::new(),
        source: EverydaySource::Average,
    };
    if average(data, &uncategorised) > 0 {
        out.push(uncategorised);
    }
    out
}

/// The average spending of a line over the history, never below 0.
fn average(data: &Data, line: &Line) -> i64 {
    if data.history.is_empty() {
        return 0;
    }
    let total: i64 = data
        .history
        .iter()
        .map(|totals| spending(totals, line))
        .sum();
    (total / data.history.len() as i64).max(0)
}

/// A line's spending in `totals`: positive when money went out.
fn spending(totals: &Totals, line: &Line) -> i64 {
    if line.category_id.is_none() {
        return -totals.uncategorised_minor;
    }
    line.covers.iter().map(|id| totals.spending_minor(id)).sum()
}

/// The Budget on `category_id` that applies in `period`: a per-Period one
/// first.
fn budget_for<'a>(data: &'a Data, category_id: &str, period: Period) -> Option<&'a Budget> {
    let (first, last) = (
        period.first_day().to_string(),
        period.last_day().to_string(),
    );
    let mut applying: Vec<&Budget> = data
        .budgets
        .iter()
        .filter(|b| {
            b.category_id == category_id
                && b.valid_from <= last
                && b.valid_to.as_ref().is_none_or(|to| *to >= first)
        })
        .collect();
    applying.sort_by_key(|b| b.per != BudgetPer::Period);
    applying.first().copied()
}

/// A line's spending in `period`.
fn line_amount(data: &Data, line: &Line, period: Period) -> i64 {
    let budget = match &line.category_id {
        Some(id) => budget_for(data, id, period),
        None => None,
    };
    let Some(budget) = budget else {
        return average(data, line);
    };
    match budget.per {
        BudgetPer::Period => budget.amount_minor,
        BudgetPer::Week => {
            let days = days_in_month(period.first_day().year(), period.first_day().month());
            budget.amount_minor * i64::from(days) / 7
        }
    }
}
