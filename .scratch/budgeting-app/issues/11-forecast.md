# 11: Forecast

**What to build:** A Forecast screen shows the projected balance at the end of the current Period and at the end of the year. The main number is for the Net position (all Accounts, with credit card debt negative), and the main current Account is shown as well. The projection starts from today's balance and adds the expected occurrences of Planned items. It then adds estimated everyday spending for each expense Category: the Category's Budget where one is set, otherwise the average of the last 3 Periods. With less than 3 Periods of history, the Forecast is marked as low confidence.

**Blocked by:** 06, 08, 10

**Status:** resolved

- [x] The Forecast shows the end of the Period and the end of the year, for the Net position and for the main Account
- [x] Expected Planned item occurrences are added on their due dates. Yearly items appear in their actual month, not as Set-asides.
- [x] Everyday spending per expense Category comes from its Budget if one exists, otherwise from the average of the last 3 complete Periods. Categories already covered by Planned items are left out to avoid counting them twice.
- [x] Transfers move money between Accounts but don't change the Net position
- [x] With less than 3 complete Periods of history, the Forecast is shown as low confidence
- [x] A simple month-by-month view of the projected Net position until the end of the year
- [x] Unit tests: a Forecast with Budgets only, with averages only, with a yearly Planned item, and the low-confidence case

## Comments

**Implemented (2026-11-22).**

- No migration: the Forecast is calculated, never stored (sync-design §1.7).
- `budget_core::forecast::for_today(store, today) -> Forecast` loads the data (Account balances from `accounts::list`, `planned::expected_occurrences(today+1, 31 Dec)`, `planned::list`, `categories::list`, `budgets::list`, `totals::for_dates` for the current Period so far and the last up to 3 complete Periods) and hands it to a private pure `calculate`. `Forecast`: `today` / `end_of_period` / `end_of_year` (`ForecastPoint { date, net_position_minor, main_account_minor }`), `main_account_name`, `months` (current Period to December: `planned_minor`, `everyday_minor`, Net position and main Account at the end of each), `everyday` (the current Period's `EverydayEstimate { category_id (None = Uncategorised), name, source: budget|average, per_period_minor, left_this_period_minor }`), `complete_periods`, `low_confidence`.
- Rules (module docs): Planned item occurrences after today hit their own Account in the month they fall due; items spread as Set-asides still hit their actual month. Everyday spending per expense Category: its Budget applying in that Period (a Budget per Week = amount × days in Period / 7; a parent Budget covers its children), otherwise the average over the last 3 complete Periods (Refunds included, never below 0; over fewer Periods if that's all there is), plus Uncategorised spending (CONTEXT.md: it counts towards the Forecast). Categories with a Planned item that hasn't ended are left out. In the current Period only what is left counts (estimate − spent so far, ≥ 0). Everyday spending is taken from the main Account. Transfers aren't projected and are already in both balances, so the Net position doesn't change; `totals::for_dates` keeps them out of the averages.
- Complete Period of history: ended before the current Period and started on or after the history's first day (the day after the earliest Opening balance, or the earliest Transaction if earlier). Fewer than 3 = low confidence.
- Ticket 09: the Forecast reads only `expected_occurrences`, so fulfilled/skipped/moved occurrences are picked up automatically once 09 lands. Overdue occurrences (due earlier this Period, still expected) are not counted, since the range starts after today (as suggested in 08's notes); after 09 it may be worth starting from the Period's first day, because fulfilled ones will drop out.
- Command `forecast_get(today)`; `forecastApi.get(today)` in `src/api.ts`.
- UI: new "Forecast" page (`src/components/ForecastPage.tsx`, second in the desktop nav, not on the phone tabs): a low-confidence notice (`role="status"`), the Net position today / end of Period / end of year with the main Account under each, a month-by-month table with an `aria-hidden` CSS bar per month (plain React + styles.css, no chart library), and the everyday spending lines with their source. It refetches on focus/visibility and on `data://changed`. `src/forecast.ts`: `netBars`, `confidenceText`.
- Open: the Savings target isn't projected as a monthly Transfer out of the main Account (it doesn't change the Net position, but the main Account forecast is higher than it will be if the user saves it). A one-sided Transfer (other side not imported) shows in today's Net position until the other side arrives. A Budget on a parent whose child has a Planned item still counts the whole Budget. The year is the calendar year of `today`.
- Tests: 12 in `core/tests/forecast.rs` (Budgets only, averages only, a yearly Planned item and Planned items on their due dates with the month view, low confidence with 1/2/3 Periods and without Accounts, Transfers, a Planned item on another Account, Uncategorised, parent and weekly Budgets, an ended Planned item, December). 8 UI tests in `src/forecast.test.ts`. In total 244 Rust tests and 116 UI tests. Not checked by launching the app, to keep test data out of the real app data folder.
