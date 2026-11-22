//! Planned items: Income or Expenses expected on a schedule. Their expected
//! occurrences are calculated from the schedule, never stored, and yearly or
//! irregular ones are spread as Set-asides (CONTEXT.md, sync-design §1.7).

use budget_core::date::Date;
use budget_core::planned::{PlannedItem, Schedule};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

fn dates(v: &[&str]) -> Vec<Date> {
    v.iter().map(|s| d(s)).collect()
}

fn item(amount_minor: i64, schedule: Schedule, start: &str, end: Option<&str>) -> PlannedItem {
    PlannedItem {
        id: "item".into(),
        account_id: "account".into(),
        category_id: "category".into(),
        description: "x".into(),
        amount_minor,
        schedule,
        start_date: d(start),
        end_date: end.map(d),
        set_aside: !schedule.is_monthly(),
    }
}

fn monthly(day: u32) -> Schedule {
    Schedule::Monthly {
        day,
        every_months: 1,
    }
}

#[test]
fn a_monthly_salary_on_the_27th_is_expected_on_the_27th_of_every_period() {
    let salary = item(250_000, monthly(27), "2026-01-01", None);
    assert_eq!(
        salary.occurrences_between(d("2026-11-01"), d("2026-11-30")),
        dates(&["2026-11-27"])
    );
    assert_eq!(
        salary.occurrences_between(d("2026-10-01"), d("2027-01-31")),
        dates(&["2026-10-27", "2026-11-27", "2026-12-27", "2027-01-27"])
    );
}

#[test]
fn a_yearly_item_is_expected_once_a_year_on_its_date() {
    let insurance = item(
        -60_000,
        Schedule::Yearly { month: 6, day: 15 },
        "2026-01-01",
        None,
    );
    assert_eq!(
        insurance.occurrences_between(d("2026-01-01"), d("2027-12-31")),
        dates(&["2026-06-15", "2027-06-15"])
    );
    assert!(insurance
        .occurrences_between(d("2026-07-01"), d("2026-07-31"))
        .is_empty());
}

#[test]
fn every_n_months_counts_from_the_month_of_the_start_date() {
    let quarterly = item(
        -30_000,
        Schedule::Monthly {
            day: 10,
            every_months: 3,
        },
        "2026-02-10",
        None,
    );
    assert_eq!(
        quarterly.occurrences_between(d("2026-01-01"), d("2027-03-31")),
        dates(&[
            "2026-02-10",
            "2026-05-10",
            "2026-08-10",
            "2026-11-10",
            "2027-02-10"
        ])
    );
}

#[test]
fn a_day_past_the_end_of_a_short_month_falls_on_its_last_day() {
    let rent = item(-90_000, monthly(31), "2026-01-01", None);
    assert_eq!(
        rent.occurrences_between(d("2027-02-01"), d("2027-04-30")),
        dates(&["2027-02-28", "2027-03-31", "2027-04-30"])
    );
    let leap = item(
        -1_000,
        Schedule::Yearly { month: 2, day: 29 },
        "2027-01-01",
        None,
    );
    assert_eq!(
        leap.occurrences_between(d("2027-01-01"), d("2028-12-31")),
        dates(&["2027-02-28", "2028-02-29"])
    );
}

#[test]
fn nothing_is_expected_before_the_start_date_or_after_the_end_date() {
    // Starts on the 28th, so November's 27th is too early; ends in March.
    let salary = item(250_000, monthly(27), "2026-11-28", Some("2027-03-15"));
    assert_eq!(
        salary.occurrences_between(d("2026-01-01"), d("2027-12-31")),
        dates(&["2026-12-27", "2027-01-27", "2027-02-27"])
    );
}

#[test]
fn a_schedule_is_stored_as_an_rrule() {
    for (schedule, rrule) in [
        (monthly(27), "FREQ=MONTHLY;BYMONTHDAY=27"),
        (
            Schedule::Monthly {
                day: 1,
                every_months: 3,
            },
            "FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=1",
        ),
        (
            Schedule::Yearly { month: 6, day: 15 },
            "FREQ=YEARLY;BYMONTH=6;BYMONTHDAY=15",
        ),
    ] {
        assert_eq!(schedule.to_rrule(), rrule);
        assert_eq!(Schedule::from_rrule(rrule).unwrap(), schedule);
    }
}

fn p(s: &str) -> budget_core::period::Period {
    s.parse().unwrap()
}

#[test]
fn a_yearly_600_item_gives_a_set_aside_of_50_per_period() {
    let insurance = item(
        -60_000,
        Schedule::Yearly { month: 6, day: 15 },
        "2026-01-01",
        None,
    );
    for period in ["2026-01", "2026-06", "2026-11", "2027-03"] {
        assert_eq!(insurance.set_aside_for(p(period)), -5_000, "{period}");
    }
}

#[test]
fn every_n_months_sets_aside_its_yearly_total_divided_by_12() {
    // 4 × 100.00 a year = 33.33… per Period, rounded so enough is set aside.
    let quarterly = item(
        -10_000,
        Schedule::Monthly {
            day: 1,
            every_months: 3,
        },
        "2026-01-01",
        None,
    );
    assert_eq!(quarterly.set_aside_for(p("2026-02")), -3_334);
}

#[test]
fn an_item_has_set_asides_only_in_the_periods_between_its_start_and_end() {
    let insurance = item(
        -60_000,
        Schedule::Yearly { month: 6, day: 15 },
        "2026-03-20",
        Some("2027-03-31"),
    );
    assert_eq!(insurance.set_aside_for(p("2026-02")), 0);
    assert_eq!(insurance.set_aside_for(p("2026-03")), -5_000);
    assert_eq!(insurance.set_aside_for(p("2027-03")), -5_000);
    assert_eq!(insurance.set_aside_for(p("2027-04")), 0);
}

#[test]
fn an_item_that_is_not_spread_has_no_set_aside() {
    let mut insurance = item(
        -60_000,
        Schedule::Yearly { month: 6, day: 15 },
        "2026-01-01",
        None,
    );
    insurance.set_aside = false;
    assert_eq!(insurance.set_aside_for(p("2026-06")), 0);
    let salary = item(250_000, monthly(27), "2026-01-01", None);
    assert_eq!(salary.set_aside_for(p("2026-06")), 0);
}

// ----- Creating, editing and ending Planned items -----

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::planned::{self, NewPlannedItem};
use budget_core::{Error, Store};

struct Fixture {
    store: Store,
    account: Account,
    salary: Category,
    insurance: Category,
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
    let salary = category("Salary", CategoryKind::Income);
    let insurance = category("Insurance", CategoryKind::Expense);
    Fixture {
        store,
        account,
        salary,
        insurance,
    }
}

impl Fixture {
    fn new_item(
        &self,
        category: &Category,
        amount_minor: i64,
        schedule: Schedule,
    ) -> NewPlannedItem {
        NewPlannedItem {
            account_id: self.account.id.clone(),
            category_id: category.id.clone(),
            description: "Planned".into(),
            amount_minor,
            schedule,
            start_date: d("2026-01-01"),
            end_date: None,
            set_aside: None,
        }
    }

    fn salary(&self) -> NewPlannedItem {
        self.new_item(&self.salary, 250_000, monthly(27))
    }

    fn insurance(&self) -> NewPlannedItem {
        self.new_item(
            &self.insurance,
            -60_000,
            Schedule::Yearly { month: 6, day: 15 },
        )
    }

    fn create(&mut self, new: NewPlannedItem) -> PlannedItem {
        planned::create(&mut self.store, new).unwrap()
    }

    fn invalid(&mut self, new: NewPlannedItem) -> String {
        match planned::create(&mut self.store, new) {
            Err(Error::Invalid(msg)) => msg,
            other => panic!("expected Invalid, got {other:?}"),
        }
    }
}

#[test]
fn the_user_creates_planned_items_and_sees_them_listed() {
    let mut f = fixture();
    let salary = f.create(f.salary());
    let insurance = f.create(f.insurance());

    assert_eq!(salary.schedule, monthly(27));
    assert_eq!(salary.amount_minor, 250_000);
    assert_eq!(salary.end_date, None);
    assert_eq!(planned::list(&f.store).unwrap(), vec![salary, insurance]);
}

#[test]
fn items_less_often_than_monthly_default_to_being_spread_as_set_asides() {
    let mut f = fixture();
    let salary = f.create(f.salary());
    let insurance = f.create(f.insurance());
    let quarterly = f.create(f.new_item(
        &f.insurance,
        -10_000,
        Schedule::Monthly {
            day: 1,
            every_months: 3,
        },
    ));

    assert!(!salary.set_aside);
    assert!(insurance.set_aside);
    assert!(quarterly.set_aside);
}

#[test]
fn the_user_can_turn_set_asides_off_for_an_item() {
    let mut f = fixture();
    let kept_whole = f.create(NewPlannedItem {
        set_aside: Some(false),
        ..f.insurance()
    });
    assert!(!kept_whole.set_aside);

    let insurance = f.create(f.insurance());
    let edited = planned::update(
        &mut f.store,
        planned::PlannedItem {
            set_aside: false,
            ..insurance
        },
    )
    .unwrap();
    assert!(!edited.set_aside);
    assert!(planned::list(&f.store).unwrap().contains(&edited));
}

#[test]
fn a_monthly_item_cannot_be_spread_as_set_asides() {
    let mut f = fixture();
    let msg = f.invalid(NewPlannedItem {
        set_aside: Some(true),
        ..f.salary()
    });
    assert!(msg.contains("less often than monthly"), "{msg}");
}

#[test]
fn the_user_edits_a_planned_item() {
    let mut f = fixture();
    let rent = f.create(f.new_item(&f.insurance, -90_000, monthly(1)));
    let edited = planned::update(
        &mut f.store,
        planned::PlannedItem {
            description: "Rent (new flat)".into(),
            amount_minor: -95_000,
            schedule: monthly(3),
            start_date: d("2026-12-01"),
            ..rent.clone()
        },
    )
    .unwrap();

    assert_eq!(edited.id, rent.id);
    assert_eq!(planned::list(&f.store).unwrap(), vec![edited]);
}

#[test]
fn the_user_ends_a_planned_item() {
    let mut f = fixture();
    let salary = f.create(f.salary());

    let ended = planned::end(&mut f.store, &salary.id, d("2027-03-31")).unwrap();

    assert_eq!(ended.end_date, Some(d("2027-03-31")));
    assert_eq!(planned::list(&f.store).unwrap(), vec![ended]);
    assert!(matches!(
        planned::end(&mut f.store, &salary.id, d("2025-12-31")),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn a_deleted_planned_item_is_gone() {
    let mut f = fixture();
    let salary = f.create(f.salary());
    planned::delete(&mut f.store, &salary.id).unwrap();
    assert!(planned::list(&f.store).unwrap().is_empty());
}

#[test]
fn the_category_decides_whether_the_amount_is_money_in_or_out() {
    let mut f = fixture();
    let msg = f.invalid(NewPlannedItem {
        amount_minor: -250_000,
        ..f.salary()
    });
    assert!(msg.contains("Income"), "{msg}");
    let msg = f.invalid(NewPlannedItem {
        amount_minor: 60_000,
        ..f.insurance()
    });
    assert!(msg.contains("Expense"), "{msg}");
}

#[test]
fn a_planned_item_needs_valid_details() {
    let mut f = fixture();
    let cases = [
        NewPlannedItem {
            description: "  ".into(),
            ..f.salary()
        },
        NewPlannedItem {
            amount_minor: 0,
            ..f.salary()
        },
        NewPlannedItem {
            account_id: "missing".into(),
            ..f.salary()
        },
        NewPlannedItem {
            category_id: "missing".into(),
            ..f.salary()
        },
        NewPlannedItem {
            end_date: Some(d("2025-12-31")),
            ..f.salary()
        },
        NewPlannedItem {
            schedule: monthly(32),
            ..f.salary()
        },
        NewPlannedItem {
            schedule: Schedule::Monthly {
                day: 1,
                every_months: 0,
            },
            ..f.salary()
        },
        NewPlannedItem {
            schedule: Schedule::Yearly { month: 2, day: 30 },
            ..f.insurance()
        },
        NewPlannedItem {
            schedule: Schedule::Yearly { month: 13, day: 1 },
            ..f.insurance()
        },
    ];
    for case in cases {
        f.invalid(case);
    }
    assert!(planned::list(&f.store).unwrap().is_empty());
}

// ----- The Period view -----

#[test]
fn the_period_view_shows_its_expected_occurrences_and_set_asides() {
    let mut f = fixture();
    let salary = f.create(f.salary());
    let insurance = f.create(f.insurance());

    let june = planned::for_period(&f.store, p("2026-06")).unwrap();

    let occurrences: Vec<_> = june
        .occurrences
        .iter()
        .map(|o| {
            (
                o.planned_item_id.as_str(),
                o.due_date,
                o.amount_minor,
                o.set_aside,
            )
        })
        .collect();
    assert_eq!(
        occurrences,
        vec![
            (insurance.id.as_str(), d("2026-06-15"), -60_000, true),
            (salary.id.as_str(), d("2026-06-27"), 250_000, false),
        ]
    );
    assert_eq!(june.occurrences[1].kind, CategoryKind::Income);
    assert_eq!(june.set_asides.len(), 1);
    assert_eq!(june.set_asides[0].planned_item_id, insurance.id);
    assert_eq!(june.set_asides[0].amount_minor, -5_000);
    assert_eq!(june.expected_income_minor(), 250_000);
    assert_eq!(june.expected_expenses_minor(), 0);
    assert_eq!(june.set_asides_minor(), -5_000);

    let july = planned::for_period(&f.store, p("2026-07")).unwrap();
    assert_eq!(july.occurrences.len(), 1);
    assert_eq!(july.set_asides_minor(), -5_000);
}

#[test]
fn expected_occurrences_are_calculated_not_stored() {
    let mut f = fixture();
    let salary = f.create(f.salary());
    let rows_before: i64 = f
        .store
        .conn()
        .query_row("SELECT COUNT(*) FROM op", [], |r| r.get(0))
        .unwrap();

    let year = planned::expected_occurrences(&f.store, d("2026-01-01"), d("2026-12-31")).unwrap();
    assert_eq!(year.len(), 12);
    assert!(year.iter().all(|o| o.planned_item_id == salary.id));

    // Editing the schedule moves every occurrence; nothing else was written.
    planned::update(
        &mut f.store,
        planned::PlannedItem {
            schedule: monthly(25),
            ..salary
        },
    )
    .unwrap();
    let year = planned::expected_occurrences(&f.store, d("2026-01-01"), d("2026-12-31")).unwrap();
    assert!(year.iter().all(|o| o.due_date.day() == 25));
    let rows_after: i64 = f
        .store
        .conn()
        .query_row("SELECT COUNT(*) FROM op", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows_after, rows_before + 1, "only the edit is logged");
}

#[test]
fn an_item_on_a_deleted_account_is_no_longer_expected() {
    let mut f = fixture();
    f.create(f.salary());
    f.create(f.insurance());
    accounts::delete(&mut f.store, &f.account.id.clone()).unwrap();

    let june = planned::for_period(&f.store, p("2026-06")).unwrap();
    assert!(june.occurrences.is_empty());
    assert!(june.set_asides.is_empty());
}

#[test]
fn an_item_whose_category_is_deleted_still_counts_as_what_it_was() {
    let mut f = fixture();
    f.create(f.salary());
    categories::delete(&mut f.store, &f.salary.id.clone()).unwrap();

    let june = planned::for_period(&f.store, p("2026-06")).unwrap();
    assert_eq!(june.expected_income_minor(), 250_000);
}

#[test]
fn a_planned_item_reaches_the_other_device_through_sync() {
    let mut f = fixture();
    f.create(f.salary());
    let insurance = f.create(f.insurance());
    planned::end(&mut f.store, &insurance.id, d("2027-12-31")).unwrap();
    let mut phone = Store::open_in_memory().unwrap();

    let ops: Vec<_> = f
        .store
        .version_vector()
        .unwrap()
        .keys()
        .flat_map(|origin| f.store.ops_since(origin, 0).unwrap())
        .collect();
    phone.apply_remote(&ops).unwrap();

    assert_eq!(
        planned::list(&phone).unwrap(),
        planned::list(&f.store).unwrap()
    );
    assert_eq!(
        planned::for_period(&phone, p("2026-06")).unwrap(),
        planned::for_period(&f.store, p("2026-06")).unwrap()
    );
}
