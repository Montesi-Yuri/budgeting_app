# 08: Planned items and Set-asides

**What to build:** The user creates Planned items: Income or Expenses expected on a schedule, such as salary every month on the 27th, rent on the 1st, or insurance every year in June. Expected occurrences are shown on a timeline and included in Spendable. Yearly or irregular Planned items are spread as Set-asides: the yearly total divided by 12 is subtracted from every Period's Spendable, and the item's own occurrence isn't subtracted again in the month it falls due.

**Blocked by:** 05

**Status:** resolved

- [x] The user can create, edit and end a Planned item: Account, Category (which decides Income or Expense), description, amount, schedule (monthly on a day, yearly on a date, every N months), start date and optional end date
- [x] Planned items that occur less often than monthly default to being spread as Set-asides. The user can turn this off per item.
- [x] The current Period's view shows its expected occurrences, along with the Set-asides
- [x] Spendable now also includes the expected Income and Expense occurrences in the Period that aren't spread, minus the Set-asides for the Period
- [x] Expected occurrences are calculated from the schedule and not stored
- [x] Unit tests: a monthly salary on the 27th, a yearly 600 item giving 50 per Period, the June Period not counting the yearly item twice, and an item that ends partway through the year

## Comments

**Implemented (2026-11-22).**

- Prep: `budgets.rs` now uses `budget_core::period` (`Period`/`Week`); its own `period_of`/`week_of` are gone (their Week test is covered by `core/tests/period.rs`).
- Migration `core/migrations/0009_planned_items.sql`: synced table `planned_item` (sync-design §1.3, with `updated_hlc`/`deleted`; `account_id`/`category_id` are not foreign keys, so remote ops apply in any order). Registered in `SYNCED_ENTITIES`. Until 0003-0008 are merged it sits fourth in `MIGRATIONS`.
- `budget_core::planned`:
  - `Schedule` = `Monthly { day, every_months }` (1 = monthly, N = every N months counted from the start date's month) or `Yearly { month, day }`. A day past the end of a short month falls on its last day (31st → 30 Nov; 29 Feb → 28 Feb outside leap years). Stored in the `rrule` column (`FREQ=MONTHLY;BYMONTHDAY=27`, `FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=1`, `FREQ=YEARLY;BYMONTH=6;BYMONTHDAY=15`; DTSTART = start_date); in op payloads and the UI it is a tagged enum.
  - `PlannedItem` (row; dates are `Date`), `create(NewPlannedItem)` (`set_aside: None` = default: spread when less often than monthly), `update(PlannedItem)` (every field, incl. end date and set_aside), `end(id, last_day)`, `delete(id)`, `list`. Rules: live Account and Category, the Category's kind decides the sign (Income > 0, Expense < 0), valid schedule, end ≥ start, and only items less often than monthly can be spread.
  - Pure: `PlannedItem::occurrences_between(from, to) -> Vec<Date>` (the one schedule expansion; respects start/end) and `PlannedItem::set_aside_for(period) -> i64` (amount / months between occurrences = yearly total / 12, signed, rounded down so enough is set aside; only in Periods overlapping start..end).
  - Store: `expected_occurrences(store, from, to) -> Vec<Occurrence>` (live items on live Accounts, by date; a deleted Category still says Income/Expense) and `for_period(store, period) -> PeriodPlan { occurrences, set_asides }` with `expected_income_minor()`, `expected_expenses_minor()` (both skip items spread as Set-asides) and `set_asides_minor()`.
- Spendable: `PeriodSpendable` gains `expected_income_minor`, `expected_expenses_minor`, `set_asides_minor` (signed, negative for Expenses). `calculate(period, &totals, &plan, savings_target, today)`; `for_period` loads the plan. An item spread as Set-asides counts only as its Set-aside, so June isn't hit twice. Weekly Spendable follows.
- Commands: `planned_items_list`, `planned_item_create`, `planned_item_update`, `planned_item_end(id, last_day)`, `planned_item_delete`, `planned_period_get(period)`.
- UI: new "Planned" page (`src/components/PlannedItemsPage.tsx`) to add, edit, end and delete items, with the Set-aside checkbox (shown only for items less often than monthly, ticked by default). Home shows the new breakdown rows and a "Planned this Period" card: the expected occurrences by date (those covered by a Set-aside are marked) and the Set-asides. `src/planned.ts` has `scheduleLabel`, `lessOftenThanMonthly`, `signedAmount`.
- For ticket 09: subtract stored `planned_occurrence` rows (fulfilled/skipped, `later` moved) inside `expected_occurrences`; occurrence id = UUIDv5(planned_item_id | due_date) (enable uuid's `v5` feature). Until then, a Manual Transaction for a planned salary is counted twice (the Transaction and the expected occurrence). Also: a Transaction fulfilling an item spread as Set-asides must be left out of the totals in Spendable.
- For ticket 11: use `expected_occurrences(store, today+1, end)` for the Forecast.
- Open question: a yearly *Income* item (e.g. a bonus) also defaults to being spread, which counts 1/12 of it as Income every Period before it arrives. The acceptance criterion says "Planned items", so that is what's built; the user can untick it.
- Tests: 24 in `core/tests/planned.rs`, 6 more in `core/tests/spendable.rs`, 5 in `src/planned.test.ts`. Not checked by launching the app, to keep test data out of the real app data folder.
