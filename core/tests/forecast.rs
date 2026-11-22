//! The Forecast (CONTEXT.md): the projected balance at the end of the
//! current Period and at the end of the year, for the Net position and for
//! the main Account. "Today" is always passed in.

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::budgets::{self, BudgetPer, NewBudget};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::date::Date;
use budget_core::forecast::{self, EverydaySource, Forecast};
use budget_core::period::Period;
use budget_core::planned::{self, NewPlannedItem, Schedule};
use budget_core::transactions::{self, NewManualTransaction, Transaction};
use budget_core::{transfers, Store};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

fn p(s: &str) -> Period {
    s.parse().unwrap()
}

/// Sunday 22 November 2026.
const TODAY: &str = "2026-11-22";

struct Fixture {
    store: Store,
    main: Account,
    groceries: Category,
    salary: Category,
}

/// A main current Account with 1000.00 on 31 July, so August, September
/// and October are complete Periods of history on 22 November.
fn fixture() -> Fixture {
    fixture_opened("2026-07-31")
}

fn fixture_opened(opening_date: &str) -> Fixture {
    let mut store = Store::open_in_memory().unwrap();
    let main = account(
        &mut store,
        "Checking",
        AccountKind::Current,
        100_000,
        opening_date,
        true,
    );
    let groceries = category(&mut store, "Groceries", CategoryKind::Expense, None);
    let salary = category(&mut store, "Salary", CategoryKind::Income, None);
    Fixture {
        store,
        main,
        groceries,
        salary,
    }
}

fn account(
    store: &mut Store,
    name: &str,
    kind: AccountKind,
    opening_minor: i64,
    opening_date: &str,
    is_main: bool,
) -> Account {
    accounts::create(
        store,
        NewAccount {
            name: name.into(),
            kind,
            opening_minor,
            opening_date: opening_date.into(),
            is_main,
        },
    )
    .unwrap()
}

fn category(
    store: &mut Store,
    name: &str,
    kind: CategoryKind,
    parent: Option<&Category>,
) -> Category {
    categories::create(
        store,
        NewCategory {
            name: name.into(),
            kind,
            parent_id: parent.map(|c| c.id.clone()),
        },
    )
    .unwrap()
}

impl Fixture {
    fn expense(&mut self, name: &str) -> Category {
        category(&mut self.store, name, CategoryKind::Expense, None)
    }

    fn txn(
        &mut self,
        account: &Account,
        date: &str,
        amount_minor: i64,
        category: Option<&Category>,
    ) -> Transaction {
        transactions::create_manual(
            &mut self.store,
            NewManualTransaction {
                account_id: account.id.clone(),
                date: date.into(),
                amount_minor,
                description: "Shop".into(),
                category_id: category.map(|c| c.id.clone()),
                note: None,
            },
        )
        .unwrap()
    }

    fn spend(&mut self, date: &str, amount_minor: i64, category: &Category) {
        let main = self.main.clone();
        self.txn(&main, date, -amount_minor, Some(category));
    }

    fn budget(&mut self, category: &Category, per: BudgetPer, amount_minor: i64) {
        budgets::create(
            &mut self.store,
            NewBudget {
                category_id: category.id.clone(),
                per,
                amount_minor,
                valid_from: "2026-01-01".into(),
            },
        )
        .unwrap();
    }

    fn planned(
        &mut self,
        category: &Category,
        amount_minor: i64,
        schedule: Schedule,
        set_aside: Option<bool>,
    ) {
        planned::create(
            &mut self.store,
            NewPlannedItem {
                account_id: self.main.id.clone(),
                category_id: category.id.clone(),
                description: category.name.clone(),
                amount_minor,
                schedule,
                start_date: d("2026-01-01"),
                end_date: None,
                set_aside,
            },
        )
        .unwrap();
    }

    fn forecast(&self) -> Forecast {
        forecast::for_today(&self.store, d(TODAY)).unwrap()
    }
}

#[test]
fn with_budgets_only_everyday_spending_is_what_is_left_of_each_budget() {
    let mut f = fixture();
    f.budget(&f.groceries.clone(), BudgetPer::Period, 40_000);
    f.spend("2026-11-10", 15_000, &f.groceries.clone());

    let forecast = f.forecast();

    // Today: 1000 − 150 = 850. This Period: the 250 left of the Budget.
    // December: the whole 400.
    assert_eq!(forecast.today.net_position_minor, 85_000);
    assert_eq!(forecast.end_of_period.date, d("2026-11-30"));
    assert_eq!(forecast.end_of_period.net_position_minor, 60_000);
    assert_eq!(forecast.end_of_year.date, d("2026-12-31"));
    assert_eq!(forecast.end_of_year.net_position_minor, 20_000);
    // The main Account is the only Account here.
    assert_eq!(forecast.main_account_name.as_deref(), Some("Checking"));
    assert_eq!(forecast.end_of_period.main_account_minor, Some(60_000));
    assert_eq!(forecast.end_of_year.main_account_minor, Some(20_000));
    assert!(!forecast.low_confidence);

    let groceries = &forecast.everyday[0];
    assert_eq!(
        groceries.category_id.as_deref(),
        Some(f.groceries.id.as_str())
    );
    assert_eq!(groceries.source, EverydaySource::Budget);
    assert_eq!(groceries.per_period_minor, 40_000);
    assert_eq!(groceries.left_this_period_minor, 25_000);
}

#[test]
fn without_budgets_everyday_spending_is_the_average_of_the_last_3_complete_periods() {
    let mut f = fixture();
    let groceries = f.groceries.clone();
    let fun = f.expense("Fun");
    // Before the Opening balance and more than 3 Periods ago: not counted.
    f.spend("2026-07-15", 99_900, &groceries);
    f.spend("2026-08-05", 30_000, &groceries);
    f.spend("2026-09-05", 33_000, &groceries);
    f.spend("2026-09-20", 6_000, &fun);
    f.spend("2026-10-05", 36_000, &groceries);
    f.spend("2026-11-10", 10_000, &groceries);

    let forecast = f.forecast();

    // Groceries average 330, of which 100 is spent; Fun averages 20.
    assert_eq!(forecast.today.net_position_minor, -15_000);
    assert_eq!(
        forecast.end_of_period.net_position_minor,
        -15_000 - 23_000 - 2_000
    );
    assert_eq!(
        forecast.end_of_year.net_position_minor,
        -40_000 - 33_000 - 2_000
    );
    assert!(!forecast.low_confidence);
    let lines: Vec<_> = forecast
        .everyday
        .iter()
        .map(|e| {
            (
                e.name.as_str(),
                e.source,
                e.per_period_minor,
                e.left_this_period_minor,
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            ("Groceries", EverydaySource::Average, 33_000, 23_000),
            ("Fun", EverydaySource::Average, 2_000, 2_000),
        ]
    );
}

#[test]
fn with_less_than_3_complete_periods_of_history_the_forecast_is_low_confidence() {
    // Opened on 15 September: only October is a complete Period.
    let mut f = fixture_opened("2026-09-15");
    let groceries = f.groceries.clone();
    f.spend("2026-09-20", 50_000, &groceries);
    f.spend("2026-10-05", 30_000, &groceries);

    let forecast = f.forecast();

    assert!(forecast.low_confidence);
    assert_eq!(forecast.complete_periods, 1);
    // The average uses the complete Periods there are: October only.
    assert_eq!(forecast.everyday[0].per_period_minor, 30_000);
}

#[test]
fn with_3_complete_periods_the_forecast_is_no_longer_low_confidence() {
    // Opened on 31 July (history from 1 August): August–October are complete.
    assert!(!fixture_opened("2026-07-31").forecast().low_confidence);
    // Opened on 1 August (history from 2 August): August is not complete.
    let forecast = fixture_opened("2026-08-01").forecast();
    assert!(forecast.low_confidence);
    assert_eq!(forecast.complete_periods, 2);
}

#[test]
fn without_any_account_the_forecast_is_zero_and_low_confidence() {
    let store = Store::open_in_memory().unwrap();
    let forecast = forecast::for_today(&store, d(TODAY)).unwrap();
    assert!(forecast.low_confidence);
    assert_eq!(forecast.end_of_year.net_position_minor, 0);
    assert_eq!(forecast.end_of_year.main_account_minor, None);
}

#[test]
fn planned_items_are_added_on_their_due_dates_and_a_yearly_item_in_its_own_month() {
    let mut f = fixture();
    let salary = f.salary.clone();
    let rent = f.expense("Rent");
    let insurance = f.expense("Insurance");
    for date in ["2026-08-01", "2026-09-01", "2026-10-01", "2026-11-01"] {
        f.spend(date, 80_000, &rent);
    }
    f.planned(
        &salary,
        200_000,
        Schedule::Monthly {
            day: 27,
            every_months: 1,
        },
        None,
    );
    f.planned(
        &rent,
        -80_000,
        Schedule::Monthly {
            day: 1,
            every_months: 1,
        },
        None,
    );
    // Spread as Set-asides for Spendable, but the money leaves in December.
    f.planned(
        &insurance,
        -60_000,
        Schedule::Yearly { month: 12, day: 15 },
        None,
    );

    let forecast = f.forecast();

    // Today: 1000 − 4 × 800 = −2200. 27 November: +2000 salary (rent for
    // November is already paid). December: −800 rent, −600 insurance,
    // +2000 salary.
    assert_eq!(forecast.today.net_position_minor, -220_000);
    assert_eq!(forecast.end_of_period.net_position_minor, -20_000);
    assert_eq!(forecast.end_of_year.net_position_minor, 40_000);
    assert_eq!(forecast.end_of_year.main_account_minor, Some(40_000));
    // Rent has a Planned item, so its history isn't counted again as
    // everyday spending.
    assert_eq!(forecast.everyday, []);

    let months: Vec<_> = forecast
        .months
        .iter()
        .map(|m| {
            (
                m.period,
                m.planned_minor,
                m.everyday_minor,
                m.net_position_minor,
            )
        })
        .collect();
    assert_eq!(
        months,
        [
            (p("2026-11"), 200_000, 0, -20_000),
            (p("2026-12"), 60_000, 0, 40_000),
        ]
    );
}

#[test]
fn a_transfer_moves_money_between_accounts_without_changing_the_net_position() {
    let mut f = fixture();
    let groceries = f.groceries.clone();
    f.spend("2026-10-05", 30_000, &groceries);
    let without = f.forecast();

    let savings = account(
        &mut f.store,
        "Savings",
        AccountKind::Savings,
        0,
        "2026-07-31",
        false,
    );
    let main = f.main.clone();
    for date in ["2026-10-10", "2026-11-10"] {
        let out = f.txn(&main, date, -50_000, None);
        let into = f.txn(&savings, date, 50_000, None);
        transfers::link(&mut f.store, &out.id, &into.id).unwrap();
    }
    let with = f.forecast();

    assert_eq!(
        with.end_of_period.net_position_minor,
        without.end_of_period.net_position_minor
    );
    assert_eq!(
        with.end_of_year.net_position_minor,
        without.end_of_year.net_position_minor
    );
    // The main Account is 1000 lower; the money is in savings.
    assert_eq!(
        with.end_of_year.main_account_minor,
        without.end_of_year.main_account_minor.map(|m| m - 100_000)
    );
    // A Transfer is not everyday spending.
    assert_eq!(with.everyday, without.everyday);
}

#[test]
fn a_planned_item_on_another_account_changes_the_net_position_but_not_the_main_account() {
    let mut f = fixture();
    let card = account(
        &mut f.store,
        "Card",
        AccountKind::CreditCard,
        -20_000,
        "2026-07-31",
        false,
    );
    let streaming = f.expense("Streaming");
    planned::create(
        &mut f.store,
        NewPlannedItem {
            account_id: card.id.clone(),
            category_id: streaming.id.clone(),
            description: "Streaming".into(),
            amount_minor: -1_000,
            schedule: Schedule::Monthly {
                day: 25,
                every_months: 1,
            },
            start_date: d("2026-01-01"),
            end_date: None,
            set_aside: None,
        },
    )
    .unwrap();

    let forecast = f.forecast();

    // Credit card debt counts as negative: 1000 − 200 = 800, then −10 in
    // November and in December.
    assert_eq!(forecast.today.net_position_minor, 80_000);
    assert_eq!(forecast.end_of_year.net_position_minor, 78_000);
    assert_eq!(forecast.end_of_year.main_account_minor, Some(100_000));
}

#[test]
fn uncategorised_spending_counts_as_everyday_spending_from_its_average() {
    let mut f = fixture();
    let main = f.main.clone();
    f.txn(&main, "2026-08-05", -9_000, None);
    f.txn(&main, "2026-11-05", -1_000, None);

    let forecast = f.forecast();

    let lines: Vec<_> = forecast
        .everyday
        .iter()
        .map(|e| {
            (
                e.category_id.clone(),
                e.name.as_str(),
                e.per_period_minor,
                e.left_this_period_minor,
            )
        })
        .collect();
    assert_eq!(lines, [(None, "Uncategorised", 3_000, 2_000)]);
    assert_eq!(
        forecast.end_of_year.net_position_minor,
        90_000 - 2_000 - 3_000
    );
}

#[test]
fn a_budget_on_a_parent_covers_its_children_and_a_weekly_budget_counts_per_day() {
    let mut f = fixture();
    let food = f.expense("Food");
    let restaurants = category(
        &mut f.store,
        "Restaurants",
        CategoryKind::Expense,
        Some(&food),
    );
    let transport = f.expense("Transport");
    f.budget(&food, BudgetPer::Period, 50_000);
    // 70 per Week: 300 in a 30-day Period, 310 in a 31-day one.
    f.budget(&transport, BudgetPer::Week, 7_000);
    // Restaurants history is covered by the Food Budget, not averaged.
    f.spend("2026-10-05", 90_000, &restaurants);
    f.spend("2026-11-05", 20_000, &restaurants);

    let forecast = f.forecast();

    let lines: Vec<_> = forecast
        .everyday
        .iter()
        .map(|e| {
            (
                e.name.as_str(),
                e.source,
                e.per_period_minor,
                e.left_this_period_minor,
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            ("Food", EverydaySource::Budget, 50_000, 30_000),
            ("Transport", EverydaySource::Budget, 30_000, 30_000),
        ]
    );
    let december = &forecast.months[1];
    assert_eq!(december.everyday_minor, 50_000 + 31_000);
}

#[test]
fn a_category_whose_planned_item_has_ended_is_estimated_again() {
    let mut f = fixture();
    let gym = f.expense("Gym");
    for date in ["2026-08-03", "2026-09-03", "2026-10-03"] {
        f.spend(date, 4_000, &gym);
    }
    let item = planned::create(
        &mut f.store,
        NewPlannedItem {
            account_id: f.main.id.clone(),
            category_id: gym.id.clone(),
            description: "Gym".into(),
            amount_minor: -4_000,
            schedule: Schedule::Monthly {
                day: 3,
                every_months: 1,
            },
            start_date: d("2026-01-01"),
            end_date: None,
            set_aside: None,
        },
    )
    .unwrap();
    assert_eq!(f.forecast().everyday, []);

    planned::end(&mut f.store, &item.id, d("2026-10-31")).unwrap();

    assert_eq!(f.forecast().everyday[0].per_period_minor, 4_000);
}

#[test]
fn in_december_the_end_of_the_period_is_the_end_of_the_year() {
    let f = fixture();
    let forecast = forecast::for_today(&f.store, d("2026-12-10")).unwrap();
    assert_eq!(forecast.months.len(), 1);
    assert_eq!(forecast.end_of_period, forecast.end_of_year);
    assert_eq!(forecast.end_of_year.date, d("2026-12-31"));
}
