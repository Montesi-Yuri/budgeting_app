//! Spendable and Weekly Spendable (CONTEXT.md, sync-design §1.7).
//!
//! Weekly Spendable = the Period's remaining Spendable / days left in the
//! Period × days left in the current Week. Both counts include today.

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::date::Date;
use budget_core::period::Period;
use budget_core::spendable::{self, weekly_spendable, PeriodSpendable};
use budget_core::transactions::{self, NewManualTransaction};
use budget_core::{settings, Store};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

#[test]
fn weekly_spendable_shares_the_remaining_spendable_over_the_days_left() {
    // Monday 16 November: 15 days left in the Period, 7 in the Week.
    let w = weekly_spendable(150_000, d("2026-11-16"));
    assert_eq!((w.days_left_in_period, w.days_left_in_week), (15, 7));
    assert_eq!(w.amount_minor, 70_000);
    assert_eq!(w.week_first_day, d("2026-11-16"));
    assert_eq!(w.week_last_day, d("2026-11-22"));
}

#[test]
fn a_week_cut_at_the_start_of_the_period_gets_only_its_own_days() {
    // Tuesday 1 December: 31 days left, the Week is cut to 1–6 December.
    let w = weekly_spendable(310_000, d("2026-12-01"));
    assert_eq!((w.days_left_in_period, w.days_left_in_week), (31, 6));
    assert_eq!(w.amount_minor, 60_000);
}

#[test]
fn a_week_cut_at_the_end_of_the_period_gets_all_that_is_left() {
    // Monday 28 December: the Week ends on Thursday 31, with the Period.
    let w = weekly_spendable(41_234, d("2026-12-28"));
    assert_eq!((w.days_left_in_period, w.days_left_in_week), (4, 4));
    assert_eq!(w.amount_minor, 41_234);
    assert_eq!(w.week_last_day, d("2026-12-31"));
}

#[test]
fn on_the_last_day_of_the_period_weekly_spendable_is_all_that_is_left() {
    // Monday 30 November is both a Week and the end of the Period.
    let w = weekly_spendable(12_345, d("2026-11-30"));
    assert_eq!((w.days_left_in_period, w.days_left_in_week), (1, 1));
    assert_eq!(w.amount_minor, 12_345);
}

#[test]
fn weekly_spendable_rounds_down_to_the_minor_unit() {
    // 100.00 over 15 days × 7 = 46.666… → 46.66; overspent: −46.67.
    assert_eq!(
        weekly_spendable(10_000, d("2026-11-16")).amount_minor,
        4_666
    );
    assert_eq!(
        weekly_spendable(-10_000, d("2026-11-16")).amount_minor,
        -4_667
    );
}

struct Fixture {
    store: Store,
    account: Account,
    groceries: Category,
    salary: Category,
}

fn fixture() -> Fixture {
    let mut store = Store::open_in_memory().unwrap();
    let account = accounts::create(
        &mut store,
        NewAccount {
            name: "Checking".into(),
            kind: AccountKind::Current,
            opening_minor: 0,
            opening_date: "2026-01-01".into(),
            is_main: true,
        },
    )
    .unwrap();
    let mut category = |name: &str, kind| {
        categories::create(
            &mut store,
            NewCategory {
                name: name.into(),
                kind,
                parent_id: None,
            },
        )
        .unwrap()
    };
    let groceries = category("Groceries", CategoryKind::Expense);
    let salary = category("Salary", CategoryKind::Income);
    Fixture {
        store,
        account,
        groceries,
        salary,
    }
}

impl Fixture {
    fn add(&mut self, date: &str, amount_minor: i64, category: Option<&Category>) {
        transactions::create_manual(
            &mut self.store,
            NewManualTransaction {
                account_id: self.account.id.clone(),
                date: date.into(),
                amount_minor,
                description: "x".into(),
                category_id: category.map(|c| c.id.clone()),
                note: None,
            },
        )
        .unwrap();
    }

    fn spend(&mut self, date: &str, amount_minor: i64) {
        let groceries = self.groceries.clone();
        self.add(date, -amount_minor, Some(&groceries));
    }

    fn salary(&mut self, date: &str, amount_minor: i64) {
        let salary = self.salary.clone();
        self.add(date, amount_minor, Some(&salary));
    }

    fn on(&self, period: &str, today: &str) -> PeriodSpendable {
        spendable::for_period(&self.store, period.parse().unwrap(), d(today)).unwrap()
    }
}

#[test]
fn spendable_is_income_plus_expenses_minus_the_savings_target() {
    let mut f = fixture();
    settings::set_savings_target(&mut f.store, 30_000).unwrap();
    f.salary("2026-11-01", 300_000);
    f.spend("2026-11-03", 50_000);
    let groceries = f.groceries.clone();
    f.add("2026-11-05", 2_000, Some(&groceries)); // a Refund

    let s = f.on("2026-11", "2026-11-21");

    assert_eq!(s.income_minor, 300_000);
    assert_eq!(s.expenses_minor, -48_000);
    assert_eq!(s.savings_target_minor, 30_000);
    assert_eq!(s.spendable_minor, 222_000);
    assert_eq!(s.period, Period::containing(d("2026-11-21")));
    assert_eq!(
        (s.first_day, s.last_day),
        (d("2026-11-01"), d("2026-11-30"))
    );
}

#[test]
fn uncategorised_transactions_count_as_expenses_or_refunds() {
    let mut f = fixture();
    f.salary("2026-11-01", 100_000);
    f.add("2026-11-02", -1_000, None);
    f.add("2026-11-03", 500, None);

    let s = f.on("2026-11", "2026-11-21");

    assert_eq!(s.uncategorised_minor, -500);
    assert_eq!(
        s.income_minor, 100_000,
        "a positive Uncategorised amount is not Income"
    );
    assert_eq!(s.spendable_minor, 99_500);
}

#[test]
fn each_period_starts_fresh_after_overspending() {
    let mut f = fixture();
    f.salary("2026-10-01", 100_000);
    f.spend("2026-10-20", 180_000); // October overspent by 800.00
    f.salary("2026-11-01", 100_000);

    assert_eq!(f.on("2026-10", "2026-11-21").spendable_minor, -80_000);
    assert_eq!(f.on("2026-11", "2026-11-21").spendable_minor, 100_000);
}

#[test]
fn the_savings_target_is_set_aside_from_every_period() {
    let mut f = fixture();
    settings::set_savings_target(&mut f.store, 30_000).unwrap();
    assert_eq!(f.on("2026-11", "2026-11-21").spendable_minor, -30_000);
    assert_eq!(f.on("2026-10", "2026-11-21").spendable_minor, -30_000);
}

#[test]
fn a_past_period_shows_its_final_spendable_without_a_weekly_spendable() {
    let mut f = fixture();
    f.salary("2026-10-01", 200_000);
    f.spend("2026-10-31", 50_000);

    let october = f.on("2026-10", "2026-11-21");

    assert_eq!(october.spendable_minor, 150_000);
    assert_eq!(october.weekly, None);
    assert!(f.on("2026-11", "2026-11-21").weekly.is_some());
}

#[test]
fn overspending_early_in_the_period_shrinks_the_later_weeks() {
    let mut f = fixture();
    f.salary("2026-11-01", 300_000);
    // Monday 2 November: 3,000.00 over 29 days, 7 of them this Week.
    let first_week = f.on("2026-11", "2026-11-02").weekly.unwrap().amount_minor;
    assert_eq!(first_week, 72_413);

    f.spend("2026-11-04", 150_000);

    // Monday 9 November: 1,500.00 left over 22 days.
    let next_week = f.on("2026-11", "2026-11-09").weekly.unwrap().amount_minor;
    assert_eq!(next_week, 47_727);
}

#[test]
fn underspending_early_in_the_period_grows_the_later_weeks() {
    let mut f = fixture();
    f.salary("2026-11-01", 300_000);
    f.spend("2026-11-04", 10_000);

    // Monday 9 November: 2,900.00 left over 22 days, against 724.13 offered
    // for the first full Week.
    let next_week = f.on("2026-11", "2026-11-09").weekly.unwrap().amount_minor;
    assert_eq!(next_week, 92_272);
}

#[test]
fn a_week_crossing_the_period_boundary_is_two_weeks() {
    let mut f = fixture();
    f.salary("2026-11-01", 100_000);
    f.spend("2026-11-10", 80_000);
    f.salary("2026-12-01", 310_000);
    f.spend("2026-11-30", 5_000);

    // Monday 30 November: the Week is cut to that one day, which gets
    // everything left in November.
    let nov = f.on("2026-11", "2026-11-30").weekly.unwrap();
    assert_eq!(
        (nov.week_first_day, nov.week_last_day),
        (d("2026-11-30"), d("2026-11-30"))
    );
    assert_eq!(nov.amount_minor, 15_000);

    // Tuesday 1 December: a fresh Period; its first Week runs to Sunday 6.
    let dec = f.on("2026-12", "2026-12-01").weekly.unwrap();
    assert_eq!(
        (dec.week_first_day, dec.week_last_day),
        (d("2026-12-01"), d("2026-12-06"))
    );
    assert_eq!(dec.amount_minor, 60_000);
}

#[test]
fn the_last_day_of_the_period_offers_all_that_is_left() {
    let mut f = fixture();
    f.salary("2026-11-01", 100_000);
    f.spend("2026-11-29", 99_000);

    let s = f.on("2026-11", "2026-11-30");

    assert_eq!(s.spendable_minor, 1_000);
    assert_eq!(s.weekly.unwrap().amount_minor, 1_000);
}

#[test]
fn spendable_is_recalculated_after_every_change() {
    let mut f = fixture();
    f.salary("2026-11-01", 100_000);
    assert_eq!(f.on("2026-11", "2026-11-21").spendable_minor, 100_000);

    f.spend("2026-11-21", 2_500);
    assert_eq!(f.on("2026-11", "2026-11-21").spendable_minor, 97_500);

    settings::set_savings_target(&mut f.store, 10_000).unwrap();
    assert_eq!(f.on("2026-11", "2026-11-21").spendable_minor, 87_500);
}
