//! Fulfilling Planned items: an imported Transaction is suggested as the
//! actual of an expected occurrence; occurrences still expected after their
//! Period are marked skipped or arrives later (CONTEXT.md "Planned item",
//! sync-design §1.3 `planned_occurrence`, §1.5 step 3, §1.7).

mod common;

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::date::Date;
use budget_core::fulfilment;
use budget_core::imports::{self, ImportProfile};
use budget_core::planned::{self, NewPlannedItem, Occurrence, PlannedItem, Schedule};
use budget_core::review::{self, LinkStatus, ReviewItem};
use budget_core::transactions::{self, NewManualTransaction, Transaction, TxnSource};
use budget_core::{settings, spendable, Store};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

struct Fixture {
    store: Store,
    current: Account,
    salary: Category,
    insurance: Category,
}

fn fixture() -> Fixture {
    let mut store = Store::open_in_memory().unwrap();
    settings::set_currency(&mut store, "EUR").unwrap();
    let current = accounts::create(
        &mut store,
        NewAccount {
            name: "Current".into(),
            kind: AccountKind::Current,
            opening_minor: 0,
            opening_date: "2026-01-01".into(),
            is_main: true,
        },
    )
    .unwrap();
    let category = |store: &mut Store, name: &str, kind| {
        categories::create(
            store,
            NewCategory {
                name: name.into(),
                kind,
                parent_id: None,
            },
        )
        .unwrap()
    };
    let salary = category(&mut store, "Salary", CategoryKind::Income);
    let insurance = category(&mut store, "Insurance", CategoryKind::Expense);
    Fixture {
        store,
        current,
        salary,
        insurance,
    }
}

impl Fixture {
    fn plan(&mut self, category: &Category, amount_minor: i64, schedule: Schedule) -> PlannedItem {
        planned::create(
            &mut self.store,
            NewPlannedItem {
                account_id: self.current.id.clone(),
                category_id: category.id.clone(),
                description: category.name.clone(),
                amount_minor,
                schedule,
                start_date: d("2026-11-01"),
                end_date: None,
                set_aside: None,
            },
        )
        .unwrap()
    }

    /// Salary of 2 500.00 on the 27th of every month.
    fn monthly_salary(&mut self) -> PlannedItem {
        let salary = self.salary.clone();
        self.plan(
            &salary,
            250_000,
            Schedule::Monthly {
                day: 27,
                every_months: 1,
            },
        )
    }

    /// Import `lines` (`date,description,amount`) into the current Account.
    fn import(&mut self, lines: &str) {
        let account_id = self.current.id.clone();
        if imports::profile_for_account(&self.store, &account_id)
            .unwrap()
            .is_none()
        {
            imports::save_profile(
                &mut self.store,
                ImportProfile {
                    name: "CSV".into(),
                    delimiter: ",".into(),
                    date_column: "date".into(),
                    date_format: "%Y-%m-%d".into(),
                    amount_column: Some("amount".into()),
                    decimal_sep: ".".into(),
                    description_columns: vec!["description".into()],
                    ..ImportProfile::draft(&account_id)
                },
            )
            .unwrap();
        }
        let csv = format!("date,description,amount\n{lines}");
        imports::import_file(&mut self.store, &account_id, "bank.csv", csv.as_bytes()).unwrap();
    }

    fn review(&self, today: &str) -> Vec<ReviewItem> {
        review::list(&self.store, d(today)).unwrap().items
    }

    fn spendable(&self, period: &str, today: &str) -> spendable::PeriodSpendable {
        spendable::for_period(&self.store, period.parse().unwrap(), d(today)).unwrap()
    }
}

#[test]
fn a_salary_arriving_2_days_late_with_a_slightly_different_amount_is_suggested_as_fulfilling_it() {
    let mut f = fixture();
    f.monthly_salary();

    f.import("2026-11-29,ACME SALARY,2480.00\n");

    let items = f.review("2026-11-30");
    let [ReviewItem::FulfilmentSuggestion { fulfilment }] = items.as_slice() else {
        panic!("expected one fulfilment Suggestion, got {items:?}");
    };
    assert_eq!(fulfilment.description, "Salary");
    assert_eq!(fulfilment.due_date, d("2026-11-27"));
    assert_eq!(fulfilment.planned_amount_minor, 250_000);
    assert_eq!(fulfilment.transaction.transaction.amount_minor, 248_000);
    // While suggested it counts as fulfilled: the actual 2 480.00 replaces
    // the planned 2 500.00, and nothing counts twice.
    let november = f.spendable("2026-11", "2026-11-30");
    assert_eq!(november.expected_income_minor, 0);
    assert_eq!(
        november.uncategorised_minor + november.income_minor,
        248_000
    );
    assert_eq!(november.spendable_minor, 248_000);
}

fn occurrence(due: &str, amount_minor: i64) -> Occurrence {
    Occurrence {
        id: "occurrence".into(),
        planned_item_id: "item".into(),
        description: "Salary".into(),
        account_id: "current".into(),
        category_id: "salary".into(),
        kind: CategoryKind::Income,
        due_date: d(due),
        scheduled_date: d(due),
        amount_minor,
        set_aside: false,
    }
}

fn txn(account_id: &str, date: &str, amount_minor: i64) -> Transaction {
    Transaction {
        id: "txn".into(),
        account_id: account_id.into(),
        date: date.into(),
        amount_minor,
        description: "ACME".into(),
        note: None,
        source: TxnSource::Import,
        import_id: None,
        fingerprint: None,
        balance_after_minor: None,
        category_id: None,
        category_source: None,
    }
}

#[test]
fn a_fulfilling_transaction_is_within_20_percent_of_the_planned_amount() {
    let salary = occurrence("2026-11-27", 250_000);
    for (amount, fulfils) in [
        (250_000, true),
        (200_000, true),
        (300_000, true),
        (199_999, false),
        (300_001, false),
        (-250_000, false), // the other sign
    ] {
        assert_eq!(
            fulfilment::could_fulfil(&salary, &txn("current", "2026-11-27", amount)),
            fulfils,
            "{amount}"
        );
    }
    let rent = occurrence("2026-11-01", -80_000);
    assert!(fulfilment::could_fulfil(
        &rent,
        &txn("current", "2026-11-01", -90_000)
    ));
    assert!(!fulfilment::could_fulfil(
        &rent,
        &txn("current", "2026-11-01", 80_000)
    ));
}

#[test]
fn a_fulfilling_transaction_is_on_the_same_account_within_5_days() {
    let salary = occurrence("2026-11-27", 250_000);
    for (date, fulfils) in [
        ("2026-11-22", true),
        ("2026-11-21", false),
        ("2026-12-02", true), // across the Period boundary
        ("2026-12-03", false),
    ] {
        assert_eq!(
            fulfilment::could_fulfil(&salary, &txn("current", date, 250_000)),
            fulfils,
            "{date}"
        );
    }
    assert!(!fulfilment::could_fulfil(
        &salary,
        &txn("savings", "2026-11-27", 250_000)
    ));
}

impl Fixture {
    fn only_fulfilment(&self, today: &str) -> fulfilment::Fulfilment {
        let fulfilments: Vec<_> = self
            .review(today)
            .into_iter()
            .filter_map(|item| match item {
                ReviewItem::FulfilmentSuggestion { fulfilment } => Some(fulfilment),
                _ => None,
            })
            .collect();
        let [one] = fulfilments.as_slice() else {
            panic!("expected one fulfilment Suggestion, got {fulfilments:?}");
        };
        one.clone()
    }

    fn still_expected(&self, today: &str) -> Vec<(String, Date)> {
        self.review(today)
            .into_iter()
            .filter_map(|item| match item {
                ReviewItem::StillExpected { occurrence } => {
                    Some((occurrence.description, occurrence.due_date))
                }
                _ => None,
            })
            .collect()
    }

    fn manual(&mut self, date: &str, amount_minor: i64) -> Transaction {
        transactions::create_manual(
            &mut self.store,
            NewManualTransaction {
                account_id: self.current.id.clone(),
                date: date.into(),
                amount_minor,
                description: "By hand".into(),
                category_id: None,
                note: None,
            },
        )
        .unwrap()
    }
}

#[test]
fn a_transaction_too_far_from_the_due_date_is_not_suggested() {
    let mut f = fixture();
    f.monthly_salary();

    f.import("2026-11-20,ACME SALARY,2500.00\n");

    assert!(f.review("2026-11-30").is_empty());
    let november = f.spendable("2026-11", "2026-11-30");
    assert_eq!(november.expected_income_minor, 250_000);
}

#[test]
fn a_confirmed_fulfilment_leaves_the_review_list_and_still_counts_once() {
    let mut f = fixture();
    f.monthly_salary();
    f.import("2026-11-26,ACME SALARY,2600.00\n");
    let suggestion = f.only_fulfilment("2026-11-30");

    fulfilment::confirm(&mut f.store, &suggestion.id).unwrap();

    assert!(f.review("2026-11-30").is_empty());
    let [linked] = fulfilment::list(&f.store).unwrap().try_into().unwrap();
    assert_eq!(linked.status, LinkStatus::Confirmed);
    assert_eq!(
        f.spendable("2026-11", "2026-11-30").spendable_minor,
        260_000
    );
}

#[test]
fn a_rejected_fulfilment_leaves_the_occurrence_expected_and_is_never_suggested_again() {
    let mut f = fixture();
    f.monthly_salary();
    f.import("2026-11-27,REFUND FROM ACME,2400.00\n");
    let suggestion = f.only_fulfilment("2026-11-30");

    fulfilment::unlink(&mut f.store, &suggestion.id).unwrap();

    assert!(fulfilment::list(&f.store).unwrap().is_empty());
    let november = f.spendable("2026-11", "2026-11-30");
    assert_eq!(november.expected_income_minor, 250_000);
    assert_eq!(november.spendable_minor, 250_000 + 240_000);

    // Device B has everything but that Transaction, then imports the same
    // file itself: the rejected pair is not suggested again.
    let txn_id = suggestion.transaction.transaction.id;
    let mut b = Store::open_in_memory().unwrap();
    let ops: Vec<_> = common::all_ops(&f.store)
        .into_iter()
        .filter(|op| op.entity_id != txn_id)
        .collect();
    b.apply_remote(&ops).unwrap();
    let mut f = Fixture { store: b, ..f };
    f.import("2026-11-27,REFUND FROM ACME,2400.00\n");
    assert!(fulfilment::list(&f.store).unwrap().is_empty());
}

#[test]
fn a_transaction_can_be_linked_to_an_occurrence_by_hand_and_unlinked() {
    let mut f = fixture();
    let salary = f.monthly_salary();
    // Far off in date and amount: only a link by hand catches it.
    let paid = f.manual("2026-11-10", 150_000);

    let linked = fulfilment::link(&mut f.store, &salary.id, d("2026-11-27"), &paid.id).unwrap();

    assert_eq!(linked.link_status, Some(LinkStatus::Confirmed));
    let november = f.spendable("2026-11", "2026-11-30");
    assert_eq!(november.expected_income_minor, 0);
    assert_eq!(november.spendable_minor, 150_000);
    assert!(f.review("2026-11-30").is_empty());

    fulfilment::unlink(&mut f.store, &linked.id).unwrap();

    let november = f.spendable("2026-11", "2026-11-30");
    assert_eq!(november.expected_income_minor, 250_000);
    assert_eq!(november.spendable_minor, 400_000);
}

#[test]
fn a_transaction_fulfils_only_one_occurrence() {
    let mut f = fixture();
    let salary = f.monthly_salary();
    let paid = f.manual("2026-11-27", 250_000);
    fulfilment::link(&mut f.store, &salary.id, d("2026-11-27"), &paid.id).unwrap();

    let again = fulfilment::link(&mut f.store, &salary.id, d("2026-12-27"), &paid.id);

    assert!(again.is_err());
}

#[test]
fn an_occurrence_still_expected_after_its_period_asks_skipped_or_arrives_later() {
    let mut f = fixture();
    f.monthly_salary();

    assert!(f.still_expected("2026-11-30").is_empty());
    assert_eq!(
        f.still_expected("2026-12-01"),
        vec![("Salary".to_owned(), d("2026-11-27"))]
    );
    assert_eq!(
        review::list(&f.store, d("2026-12-01"))
            .unwrap()
            .pending_count,
        1
    );
}

#[test]
fn a_skipped_occurrence_is_no_longer_expected() {
    let mut f = fixture();
    let salary = f.monthly_salary();

    fulfilment::skip(&mut f.store, &salary.id, d("2026-11-27")).unwrap();

    assert!(f.still_expected("2026-12-01").is_empty());
    let november = f.spendable("2026-11", "2026-12-01");
    assert_eq!(november.expected_income_minor, 0);
    assert_eq!(november.spendable_minor, 0);
    // December's occurrence is still expected.
    assert_eq!(
        f.spendable("2026-12", "2026-12-01").expected_income_minor,
        250_000
    );
}

#[test]
fn an_occurrence_that_arrives_later_moves_to_its_new_date_in_the_next_period() {
    let mut f = fixture();
    let salary = f.monthly_salary();

    fulfilment::arrives_later(&mut f.store, &salary.id, d("2026-11-27"), d("2026-12-03")).unwrap();

    assert!(f.still_expected("2026-12-01").is_empty());
    assert_eq!(
        f.spendable("2026-11", "2026-12-01").expected_income_minor,
        0
    );
    let december = planned::for_period(&f.store, "2026-12".parse().unwrap()).unwrap();
    let dates: Vec<_> = december.occurrences.iter().map(|o| o.due_date).collect();
    assert_eq!(dates, vec![d("2026-12-03"), d("2026-12-27")]);
    assert_eq!(december.occurrences[0].scheduled_date, d("2026-11-27"));
    assert_eq!(december.expected_income_minor(), 500_000);

    // It arrives on 4 December: it fulfils the moved occurrence, not
    // the one December has of its own.
    f.import("2026-12-04,ACME SALARY,2500.00\n");
    let suggestion = f.only_fulfilment("2026-12-05");
    assert_eq!(suggestion.due_date, d("2026-11-27"));
    let december = f.spendable("2026-12", "2026-12-05");
    assert_eq!(december.expected_income_minor, 250_000);
    assert_eq!(december.spendable_minor, 500_000);
}

#[test]
fn an_occurrence_moved_and_still_expected_after_its_new_period_asks_again() {
    let mut f = fixture();
    let salary = f.monthly_salary();
    fulfilment::arrives_later(&mut f.store, &salary.id, d("2026-11-27"), d("2026-12-03")).unwrap();

    assert_eq!(
        f.still_expected("2027-01-02"),
        vec![
            ("Salary".to_owned(), d("2026-12-03")),
            ("Salary".to_owned(), d("2026-12-27"))
        ]
    );
    fulfilment::skip(&mut f.store, &salary.id, d("2026-11-27")).unwrap();
    assert_eq!(
        f.still_expected("2027-01-02"),
        vec![("Salary".to_owned(), d("2026-12-27"))]
    );
}

#[test]
fn arrives_later_needs_a_date_after_the_due_date() {
    let mut f = fixture();
    let salary = f.monthly_salary();

    assert!(
        fulfilment::arrives_later(&mut f.store, &salary.id, d("2026-11-27"), d("2026-11-27"))
            .is_err()
    );
    // Not a date the item is due on.
    assert!(fulfilment::skip(&mut f.store, &salary.id, d("2026-11-26")).is_err());
}

#[test]
fn a_fulfilment_whose_transaction_is_deleted_leaves_the_occurrence_expected() {
    let mut f = fixture();
    let salary = f.monthly_salary();
    let paid = f.manual("2026-11-27", 250_000);
    fulfilment::link(&mut f.store, &salary.id, d("2026-11-27"), &paid.id).unwrap();

    transactions::delete(&mut f.store, &paid.id).unwrap();

    assert_eq!(
        f.spendable("2026-11", "2026-11-30").expected_income_minor,
        250_000
    );
}

#[test]
fn a_transaction_fulfilling_an_item_spread_as_set_asides_counts_only_as_its_set_aside() {
    let mut f = fixture();
    let insurance = f.insurance.clone();
    // 600.00 every year on 15 December: 50.00 set aside every Period.
    f.plan(&insurance, -60_000, Schedule::Yearly { month: 12, day: 15 });
    assert_eq!(f.spendable("2026-12", "2026-12-01").spendable_minor, -5_000);

    f.import("2026-12-14,ALLIANZ,-612.00\n");

    let suggestion = f.only_fulfilment("2026-12-20");
    assert_eq!(suggestion.description, "Insurance");
    let december = f.spendable("2026-12", "2026-12-20");
    assert_eq!(december.uncategorised_minor, -61_200);
    assert_eq!(december.covered_by_set_asides_minor, 61_200);
    assert_eq!(december.set_asides_minor, -5_000);
    assert_eq!(december.spendable_minor, -5_000);
}

#[test]
fn a_transaction_in_a_transfer_is_not_suggested_as_fulfilling() {
    let mut f = fixture();
    f.monthly_salary();
    let savings = accounts::create(
        &mut f.store,
        NewAccount {
            name: "Savings".into(),
            kind: AccountKind::Savings,
            opening_minor: 0,
            opening_date: "2026-01-01".into(),
            is_main: false,
        },
    )
    .unwrap();
    transactions::create_manual(
        &mut f.store,
        NewManualTransaction {
            account_id: savings.id,
            date: "2026-11-27".into(),
            amount_minor: -250_000,
            description: "To current".into(),
            category_id: None,
            note: None,
        },
    )
    .unwrap();

    f.import("2026-11-27,FROM SAVINGS,2500.00\n");

    assert!(fulfilment::list(&f.store).unwrap().is_empty());
}

#[test]
fn the_same_occurrence_changed_on_two_devices_merges_into_one_row() {
    let mut a = fixture();
    let salary = a.monthly_salary();
    let mut b = Store::open_in_memory().unwrap();
    common::sync_pair(&mut a.store, &mut b).0.unwrap();

    // B decides after A.
    a.store.set_wall_clock(|| 1_900_000_000_000);
    b.set_wall_clock(|| 1_900_000_100_000);
    fulfilment::skip(&mut a.store, &salary.id, d("2026-11-27")).unwrap();
    fulfilment::arrives_later(&mut b, &salary.id, d("2026-11-27"), d("2026-12-05")).unwrap();
    common::sync_pair(&mut a.store, &mut b).0.unwrap();

    let rows: Vec<_> = common::synced_rows(&b)
        .into_iter()
        .filter(|r| r.starts_with("planned_occurrence:"))
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(common::synced_rows(&a.store), common::synced_rows(&b));
    // The later change (B's) wins on both Devices.
    let expected =
        planned::expected_occurrences(&a.store, d("2026-12-01"), d("2026-12-31")).unwrap();
    assert_eq!(expected[0].due_date, d("2026-12-05"));
}
