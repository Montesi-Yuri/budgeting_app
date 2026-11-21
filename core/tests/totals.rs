//! Income and Expense are decided by the Category, not by the sign of the
//! amount; a Refund (positive, in an expense Category) reduces that
//! Category's spending and is not Income (CONTEXT.md).

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::totals;
use budget_core::transactions::{self, NewManualTransaction};
use budget_core::Store;

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
            opening_date: "2026-10-31".into(),
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

    fn november(&self) -> totals::Totals {
        totals::for_dates(&self.store, "2026-11-01", "2026-11-30").unwrap()
    }
}

#[test]
fn a_refund_reduces_its_categorys_spending_and_is_not_income() {
    let mut f = fixture();
    let groceries = f.groceries.clone();
    f.add("2026-11-03", -5_000, Some(&groceries));
    f.add("2026-11-10", 2_000, Some(&groceries)); // returned purchase

    let t = f.november();

    assert_eq!(t.spending_minor(&groceries.id), 3_000);
    assert_eq!(t.expenses_minor, -3_000);
    assert_eq!(t.income_minor, 0);
}

#[test]
fn a_refund_alone_is_negative_spending_not_income() {
    let mut f = fixture();
    let groceries = f.groceries.clone();
    f.add("2026-11-10", 1_500, Some(&groceries)); // a friend pays back their share

    let t = f.november();

    assert_eq!(t.spending_minor(&groceries.id), -1_500);
    assert_eq!(t.expenses_minor, 1_500);
    assert_eq!(t.income_minor, 0);
}

#[test]
fn income_is_decided_by_an_income_category_whatever_the_sign() {
    let mut f = fixture();
    let salary = f.salary.clone();
    f.add("2026-11-27", 250_000, Some(&salary));
    f.add("2026-11-28", -10_000, Some(&salary)); // employer correction

    let t = f.november();

    assert_eq!(t.income_minor, 240_000);
    assert_eq!(t.expenses_minor, 0);
    assert_eq!(
        t.spending_minor(&salary.id),
        0,
        "an income Category has no spending"
    );
}

#[test]
fn uncategorised_transactions_are_counted_apart_and_only_dates_in_range_count() {
    let mut f = fixture();
    let groceries = f.groceries.clone();
    f.add("2026-11-01", -700, None);
    f.add("2026-11-30", 300, None);
    f.add("2026-10-31", -9_000, Some(&groceries));
    f.add("2026-12-01", -9_000, Some(&groceries));

    let t = f.november();

    assert_eq!(t.uncategorised_minor, -400);
    assert_eq!((t.income_minor, t.expenses_minor), (0, 0));
    assert_eq!(t.spending_minor(&groceries.id), 0);
}
