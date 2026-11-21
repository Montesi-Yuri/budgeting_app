//! Periods are calendar months; Weeks run Monday to Sunday and are cut at
//! the Period boundary (CONTEXT.md).

use budget_core::date::Date;
use budget_core::period::{Period, Week};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

#[test]
fn a_period_is_the_calendar_month_containing_the_date() {
    let p = Period::containing(d("2026-11-21"));
    assert_eq!(p.to_string(), "2026-11");
    assert_eq!(p.first_day(), d("2026-11-01"));
    assert_eq!(p.last_day(), d("2026-11-30"));
}

fn week(today: &str) -> (String, String) {
    let w = Week::containing(d(today));
    (w.first_day().to_string(), w.last_day().to_string())
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

#[test]
fn a_week_runs_monday_to_sunday() {
    // Saturday 21 November 2026
    assert_eq!(week("2026-11-21"), pair("2026-11-16", "2026-11-22"));
    assert_eq!(week("2026-11-16"), pair("2026-11-16", "2026-11-22"));
    assert_eq!(week("2026-11-22"), pair("2026-11-16", "2026-11-22"));
}

#[test]
fn a_week_is_cut_at_the_start_of_the_period() {
    // 1 November 2026 is a Sunday: the Period's first Week is one day long.
    assert_eq!(week("2026-11-01"), pair("2026-11-01", "2026-11-01"));
    // 1 December 2026 is a Tuesday.
    assert_eq!(week("2026-12-03"), pair("2026-12-01", "2026-12-06"));
}

#[test]
fn a_week_is_cut_at_the_end_of_the_period() {
    // Monday 30 November 2026 is the last day of its Period.
    assert_eq!(week("2026-11-30"), pair("2026-11-30", "2026-11-30"));
    // Thursday 31 December 2026, also the end of the year.
    assert_eq!(week("2026-12-29"), pair("2026-12-28", "2026-12-31"));
}

#[test]
fn february_of_a_leap_year_ends_on_the_29th() {
    let p = Period::containing(d("2028-02-10"));
    assert_eq!(p.last_day(), d("2028-02-29"));
    assert_eq!(
        Period::containing(d("2027-02-10")).last_day(),
        d("2027-02-28")
    );
    // 29 February 2028 is a Tuesday.
    assert_eq!(week("2028-02-29"), pair("2028-02-28", "2028-02-29"));
    assert_eq!(p.days_left(d("2028-02-28")), 2);
}

#[test]
fn days_left_count_today() {
    let p = Period::containing(d("2026-11-21"));
    assert_eq!(p.days_left(d("2026-11-01")), 30);
    assert_eq!(p.days_left(d("2026-11-21")), 10);
    assert_eq!(p.days_left(d("2026-11-30")), 1);
    assert_eq!(
        Week::containing(d("2026-11-21")).days_left(d("2026-11-21")),
        2
    );
    assert_eq!(
        Week::containing(d("2026-11-16")).days_left(d("2026-11-16")),
        7
    );
}

#[test]
fn periods_step_across_the_year_boundary() {
    let jan: Period = "2027-01".parse().unwrap();
    assert_eq!(jan.previous().to_string(), "2026-12");
    assert_eq!(jan.previous().next(), jan);
    assert_eq!(jan.first_day(), d("2027-01-01"));
}

#[test]
fn a_period_is_written_as_year_and_month() {
    for bad in [
        "",
        "2026",
        "2026-13",
        "2026-00",
        "2026-1",
        "26-11",
        "2026-11-01",
    ] {
        assert!(bad.parse::<Period>().is_err(), "{bad:?}");
    }
}
