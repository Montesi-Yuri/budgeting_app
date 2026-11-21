//! Budgets: an optional spending limit for one expense Category per Period
//! or per Week. A Budget on a parent Category covers its children, and a
//! Budget only warns: it never reserves money (CONTEXT.md).

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::budgets::{
    self, Budget, BudgetEdit, BudgetPer, BudgetProgress, BudgetState, DateRange, NewBudget,
};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::store::OpKind;
use budget_core::totals;
use budget_core::transactions::{self, NewManualTransaction};
use budget_core::{Error, Store};

fn range(from: &str, to: &str) -> DateRange {
    DateRange {
        from: from.into(),
        to: to.into(),
    }
}

struct Fixture {
    store: Store,
    account: Account,
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
    Fixture { store, account }
}

impl Fixture {
    fn category(&mut self, name: &str, kind: CategoryKind, parent: Option<&Category>) -> Category {
        categories::create(
            &mut self.store,
            NewCategory {
                name: name.into(),
                kind,
                parent_id: parent.map(|p| p.id.clone()),
            },
        )
        .unwrap()
    }

    fn expense(&mut self, name: &str) -> Category {
        self.category(name, CategoryKind::Expense, None)
    }

    fn spend(&mut self, date: &str, amount_minor: i64, category: &Category) {
        transactions::create_manual(
            &mut self.store,
            NewManualTransaction {
                account_id: self.account.id.clone(),
                date: date.into(),
                amount_minor,
                description: "x".into(),
                category_id: Some(category.id.clone()),
                note: None,
            },
        )
        .unwrap();
    }

    fn budget(
        &mut self,
        category: &Category,
        per: BudgetPer,
        amount_minor: i64,
    ) -> budgets::Budget {
        budgets::create(
            &mut self.store,
            NewBudget {
                category_id: category.id.clone(),
                per,
                amount_minor,
                valid_from: "2026-01-01".into(),
            },
        )
        .unwrap()
    }

    fn progress(&self, today: &str) -> Vec<BudgetProgress> {
        budgets::progress(&self.store, today).unwrap()
    }
}

#[test]
fn a_period_budget_shows_what_was_spent_and_what_is_left_this_period() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    let budget = f.budget(&groceries, BudgetPer::Period, 40_000);
    f.spend("2026-10-31", -9_999, &groceries); // last Period
    f.spend("2026-11-02", -12_000, &groceries);
    f.spend("2026-11-20", -6_000, &groceries);
    f.spend("2026-11-21", 2_000, &groceries); // a Refund
    f.spend("2026-11-25", -1_000, &groceries); // later this Period: already counted

    let p = f.progress("2026-11-21");

    assert_eq!(p.len(), 1);
    assert_eq!(p[0].budget, budget);
    assert_eq!(p[0].range, Some(range("2026-11-01", "2026-11-30")));
    assert_eq!(p[0].spent_minor, 17_000);
    assert_eq!(p[0].left_minor, 23_000);
    assert!(!p[0].over);
}

#[test]
fn a_budget_on_a_parent_category_counts_the_spending_of_all_its_children() {
    let mut f = fixture();
    let food = f.expense("Food");
    let groceries = f.category("Groceries", CategoryKind::Expense, Some(&food));
    let restaurants = f.category("Restaurants", CategoryKind::Expense, Some(&food));
    let home = f.expense("Home");
    f.budget(&food, BudgetPer::Period, 50_000);
    f.budget(&groceries, BudgetPer::Period, 30_000);
    f.spend("2026-11-03", -1_000, &food); // on the parent itself
    f.spend("2026-11-04", -20_000, &groceries);
    f.spend("2026-11-05", -15_000, &restaurants);
    f.spend("2026-11-06", 500, &restaurants); // a Refund
    f.spend("2026-11-07", -99_000, &home); // another parent: not counted

    let p = f.progress("2026-11-21");

    let spent: Vec<(String, i64)> = p
        .iter()
        .map(|b| (b.budget.category_id.clone(), b.spent_minor))
        .collect();
    assert_eq!(
        spent,
        vec![(food.id.clone(), 35_500), (groceries.id.clone(), 20_000)],
        "the parent's Budget covers itself and its children; a child's Budget covers only the child"
    );
}

#[test]
fn a_weekly_budget_counts_only_the_current_week_cut_at_the_period_boundary() {
    let mut f = fixture();
    let coffee = f.expense("Coffee");
    f.budget(&coffee, BudgetPer::Week, 2_000);
    // Monday 30 November is the whole last Week of November; the rest of
    // that calendar week (1-6 December) belongs to December's first Week.
    f.spend("2026-11-29", -700, &coffee); // the previous Week
    f.spend("2026-11-30", -300, &coffee);
    f.spend("2026-12-01", -400, &coffee);
    f.spend("2026-12-06", -900, &coffee);
    f.spend("2026-12-07", -5_000, &coffee); // the next Week

    let last_of_november = &f.progress("2026-11-30")[0];
    assert_eq!(
        last_of_november.range,
        Some(range("2026-11-30", "2026-11-30"))
    );
    assert_eq!(last_of_november.spent_minor, 300);

    let first_of_december = &f.progress("2026-12-04")[0];
    assert_eq!(
        first_of_december.range,
        Some(range("2026-12-01", "2026-12-06"))
    );
    assert_eq!(first_of_december.spent_minor, 1_300);
    assert_eq!(first_of_december.left_minor, 700);
    assert!(!first_of_december.over);
}

#[test]
fn a_budget_that_is_over_its_limit_is_flagged() {
    let mut f = fixture();
    let fun = f.expense("Fun");
    let exactly = f.expense("Exactly");
    f.budget(&fun, BudgetPer::Period, 10_000);
    f.budget(&exactly, BudgetPer::Period, 10_000);
    f.spend("2026-11-02", -10_001, &fun);
    f.spend("2026-11-02", -10_000, &exactly);

    let p = f.progress("2026-11-21");

    assert_eq!((p[0].left_minor, p[0].over), (-1, true));
    assert_eq!(
        (p[1].left_minor, p[1].over),
        (0, false),
        "reaching the limit is not over it"
    );
}

#[test]
fn a_budget_needs_a_live_expense_category_a_positive_amount_and_a_start_date() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    let salary = f.category("Salary", CategoryKind::Income, None);
    let new = |category_id: &str, amount_minor: i64, valid_from: &str| NewBudget {
        category_id: category_id.into(),
        per: BudgetPer::Period,
        amount_minor,
        valid_from: valid_from.into(),
    };

    for bad in [
        new(&salary.id, 10_000, "2026-11-01"),
        new("no-such-category", 10_000, "2026-11-01"),
        new(&groceries.id, 0, "2026-11-01"),
        new(&groceries.id, -5, "2026-11-01"),
        new(&groceries.id, 10_000, "2026-11-31"),
    ] {
        let err = budgets::create(&mut f.store, bad.clone()).unwrap_err();
        assert!(matches!(err, Error::Invalid(_)), "{bad:?} gave {err:?}");
    }
    assert!(budgets::list(&f.store).unwrap().is_empty());
}

#[test]
fn creating_a_budget_is_logged_for_sync() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    let ops_before = f.store.ops_since(f.store.device_id(), 0).unwrap().len();

    let budget = f.budget(&groceries, BudgetPer::Week, 5_000);

    let ops = f.store.ops_since(f.store.device_id(), 0).unwrap();
    assert_eq!(ops.len(), ops_before + 1);
    let op = ops.last().unwrap();
    assert_eq!((op.entity.as_str(), op.kind), ("budget", OpKind::Upsert));
    assert_eq!(op.decode_payload::<Budget>().unwrap(), Some(budget.clone()));
    assert_eq!(budgets::list(&f.store).unwrap(), vec![budget]);
}

#[test]
fn the_user_can_edit_a_budgets_amount_per_and_start_date() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    let budget = f.budget(&groceries, BudgetPer::Period, 40_000);

    let edited = budgets::update(
        &mut f.store,
        BudgetEdit {
            id: budget.id.clone(),
            category_id: groceries.id.clone(),
            per: BudgetPer::Week,
            amount_minor: 9_000,
            valid_from: "2026-11-16".into(),
        },
    )
    .unwrap();

    assert_eq!(
        edited,
        Budget {
            per: BudgetPer::Week,
            amount_minor: 9_000,
            valid_from: "2026-11-16".into(),
            ..budget
        }
    );
    assert_eq!(budgets::list(&f.store).unwrap(), vec![edited]);
}

#[test]
fn an_ended_budget_applies_up_to_its_last_day_and_then_stops() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    let budget = f.budget(&groceries, BudgetPer::Period, 40_000);

    let ended = budgets::end(&mut f.store, &budget.id, "2026-11-21").unwrap();

    assert_eq!(ended.valid_to.as_deref(), Some("2026-11-21"));
    assert_eq!(f.progress("2026-11-21")[0].state, BudgetState::Active);
    let later = &f.progress("2026-11-22")[0];
    assert_eq!(
        (later.state, later.range.clone()),
        (BudgetState::Ended, None)
    );
    assert!(matches!(
        budgets::end(&mut f.store, &budget.id, "2025-12-31"),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn a_budget_whose_category_is_deleted_is_broken_and_not_applied() {
    let mut f = fixture();
    let food = f.expense("Food");
    let groceries = f.category("Groceries", CategoryKind::Expense, Some(&food));
    let home = f.expense("Home");
    let budget = f.budget(&groceries, BudgetPer::Period, 40_000);
    f.spend("2026-11-02", -12_000, &groceries);

    categories::delete(&mut f.store, &food.id).unwrap(); // and its children

    let p = &f.progress("2026-11-21")[0];
    assert_eq!(p.budget, budget, "the Budget stays");
    assert_eq!(
        (p.state, p.range.clone(), p.over),
        (BudgetState::Broken, None, false)
    );

    // Choosing a live Category repairs it.
    budgets::update(
        &mut f.store,
        BudgetEdit {
            id: budget.id.clone(),
            category_id: home.id.clone(),
            per: budget.per,
            amount_minor: budget.amount_minor,
            valid_from: budget.valid_from.clone(),
        },
    )
    .unwrap();
    assert_eq!(f.progress("2026-11-21")[0].state, BudgetState::Active);
}

#[test]
fn an_upcoming_budget_is_not_counted_yet() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    budgets::create(
        &mut f.store,
        NewBudget {
            category_id: groceries.id.clone(),
            per: BudgetPer::Period,
            amount_minor: 40_000,
            valid_from: "2026-12-01".into(),
        },
    )
    .unwrap();
    f.spend("2026-11-02", -12_000, &groceries);

    let p = &f.progress("2026-11-21")[0];

    assert_eq!(
        (p.state, p.range.clone(), p.spent_minor),
        (BudgetState::Upcoming, None, 0)
    );
}

#[test]
fn a_budget_entered_by_mistake_can_be_deleted() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    let budget = f.budget(&groceries, BudgetPer::Period, 40_000);

    budgets::delete(&mut f.store, &budget.id).unwrap();

    assert!(budgets::list(&f.store).unwrap().is_empty());
    assert!(f.progress("2026-11-21").is_empty());
    let op = f
        .store
        .ops_since(f.store.device_id(), 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!((op.entity.as_str(), op.kind), ("budget", OpKind::Delete));
}

#[test]
fn a_budget_starting_mid_period_counts_that_whole_period() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    budgets::create(
        &mut f.store,
        NewBudget {
            category_id: groceries.id.clone(),
            per: BudgetPer::Period,
            amount_minor: 40_000,
            valid_from: "2026-11-21".into(),
        },
    )
    .unwrap();
    f.spend("2026-11-02", -12_000, &groceries);

    let p = &f.progress("2026-11-21")[0];

    assert_eq!((p.state, p.spent_minor), (BudgetState::Active, 12_000));
}

#[test]
fn budgets_reserve_no_money_the_periods_totals_are_unchanged() {
    let mut f = fixture();
    let groceries = f.expense("Groceries");
    f.spend("2026-11-02", -12_000, &groceries);
    let before = totals::for_dates(&f.store, "2026-11-01", "2026-11-30").unwrap();

    f.budget(&groceries, BudgetPer::Period, 40_000);
    f.budget(&groceries, BudgetPer::Week, 5_000);

    assert_eq!(
        totals::for_dates(&f.store, "2026-11-01", "2026-11-30").unwrap(),
        before
    );
}
