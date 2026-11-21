# 05: Spendable (basic)

**What to build:** A home screen shows Spendable for the current Period (calendar month) and the Weekly Spendable. In this first version Spendable is: Income in the Period, plus Expenses in the Period (negative, with Refunds included), minus the Savings target. The Savings target is a fixed amount per Period, set in settings. Weekly Spendable is the Period's remaining Spendable, divided by the days left in the Period and multiplied by the days left in the current Week. Weeks run Monday to Sunday and are cut at the Period boundary.

**Blocked by:** 02

**Status:** resolved

- [x] The Savings target can be set in settings and is synced
- [x] The home screen shows Spendable for the current Period, and the Weekly Spendable for the current Week
- [x] Spendable and Weekly Spendable are recalculated whenever data changes
- [x] Each Period starts fresh: overspending in one Period does not carry into the next Period's Spendable
- [x] Uncategorised Transactions count towards Spendable. Without a Category they can't be classified as Income, so they are treated as Expenses (negative) or Refunds (positive).
- [x] The user can look at past Periods' Spendable
- [x] Unit tests for the calculation, including: a Week that crosses a Period boundary, overspending early in the Period (later Weeks shrink), underspending (later Weeks grow), and the last day of the Period

## Comments

**Implemented (2026-11-21).**

- `budget_core::date::Date`: a real calendar date type (`YYYY-MM-DD`, ordered, `add_days`, `days_until`, `days_from_monday`, serde as text, leap years via `is_leap_year` / `days_in_month`).
- `budget_core::period`: `Period` (calendar month, `YYYY-MM`; `containing`, `first_day`, `last_day`, `previous`, `next`, `days_left(today)`) and `Week` (`Week::containing(date)` is Monday to Sunday cut at the Period boundary; `days_left(today)`). Pure; "today" is always passed in.
- `budget_core::spendable`: `weekly_spendable(remaining_minor, today)` (pure: remaining / days left in the Period × days left in the Week, both counting today, rounded down to the minor unit), `PeriodSpendable::calculate(period, &Totals, savings_target_minor, today)` (pure) and `for_period(store, period, today)` (reads `totals::for_dates` and the Savings target). Spendable = Income + Expenses (Refunds included) + Uncategorised − Savings target, from the Period's own Transactions only, so each Period starts fresh. `weekly` is only set when today is in the Period; a past Period shows its final Spendable.
- Later tickets: 06 and 07 exclude Transfers / matched Manual Transactions in `totals::for_dates` (Spendable follows). 08 adds new terms (expected Planned item occurrences, Set-asides) as fields on `PeriodSpendable`, loaded in `for_period` and added in `calculate`. Weekly Spendable always derives from `spendable_minor`.
- Savings target: synced setting `savings_target_minor` (sync-design §1.3), `settings::savings_target_minor` (0 when unset) and `settings::set_savings_target` (not negative), through the single write path. There is one value, so past Periods use the current Savings target (no history).
- Commands: `savings_target_get`, `savings_target_set`, `spendable_get(period, today)`. The UI passes its local date as `today`, so the Week and Period follow the user's clock.
- UI: a new Home page (default view, first in the nav) with Spendable, the Weekly Spendable with its Week and days left, the breakdown, and Previous/Next buttons to look at past Periods. It refetches on mount (every visit), on window focus/visibility, on a `data://changed` event (for Sync, ticket 13), and follows the date across midnight. A Savings target card is in Settings. `src/periods.ts` holds the tested Period/date helpers.
- Tests: 15 in `core/tests/spendable.rs`, 8 in `core/tests/period.rs`, 3 more in `core/tests/settings.rs`; 5 more UI tests in `src/periods.test.ts`. Not checked by launching the app, to keep test data out of the real app data folder.
