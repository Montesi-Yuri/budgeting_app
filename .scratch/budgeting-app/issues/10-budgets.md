# 10: Budgets

**What to build:** The user sets optional Budgets: a spending limit for a Category, per Period or per Week. A Budget on a parent Category covers its children. Budgets show progress and warn when a Category goes over, but they don't reserve money: Spendable still covers all spending.

**Blocked by:** 02

**Status:** resolved

- [x] The user can create, edit and end a Budget on an expense Category: an amount, per Period or per Week, and the date it starts applying
- [x] A Budget view shows, for the current Period or Week, the amount spent and the amount left for each Budget. Refunds reduce the amount spent. Transfers are not included.
- [x] A Budget on a parent Category counts the spending of all its children
- [x] A Budget that is over its limit is clearly flagged
- [x] Spendable is not affected by Budgets
- [x] A Budget whose Category is deleted is shown as broken
- [x] Unit tests for parent/child totals and for weekly Budgets in a Week cut at the Period boundary

## Comments

**Implemented (2026-11-21).**

- Migration `core/migrations/0007_budgets.sql` adds the synced table `budget` (sync-design §1.3, with `updated_hlc` and `deleted`; `per` is `'period' | 'week'`). `category_id` is not a foreign key, so remote ops apply in any order. The number 0007 is reserved for this ticket; until 0003-0006 are merged it sits third in `MIGRATIONS`.
- `budget_core::budgets`: `create(NewBudget)`, `update(BudgetEdit)` (Category, amount, per, start date; picking a live Category repairs a broken Budget), `end(id, last_day)` (sets `valid_to`), `delete(id)` (for a Budget entered by mistake), `list`, and `progress(today)`, which returns every live Budget with `state` (`active` / `upcoming` / `ended` / `broken`), the current Period or Week `range`, `spent_minor`, `left_minor` and `over`. A Budget must be on a live expense Category with a positive amount.
- Spending comes from `totals::for_dates`, so Refunds reduce it, and a Budget on a parent Category adds up the parent and all its children. A Budget that applies today counts its whole current Period or Week, even if it started partway through. Reaching the limit exactly is not over.
- Transfers: `totals::for_dates` does not exclude Transfers yet (ticket 06). Budgets use it, so they will leave Transfers out as soon as `totals` does.
- Spendable: Budgets don't write anything Spendable reads. A test checks that adding Budgets doesn't change the Period's totals. Ticket 05 must not subtract Budgets.
- Broken: a Budget whose Category is deleted (deleting a parent also deletes its children) stays, is shown as broken, and is not applied (sync-design §6).
- `budgets::period_of(day)` and `budgets::week_of(day)` (Monday to Sunday, cut at the Period boundary) are small self-contained helpers, with "today" passed in. Merge them with ticket 05's Period/Week helpers when both land.
- UI: new Budgets page (`src/components/BudgetsPage.tsx`) with a progress bar per Budget, an "Over by ..." flag, a "Broken" flag, and forms to add, edit, end and delete. `src/budgets.ts` has `budgetMeter` (a bar is "near" from 90% of the limit). The UI passes the local date as `today`.
- Sync (ticket 13): register the entity `budget` (`budgets::Budget`) for `apply_remote`.
- Tests: 14 new Rust tests in `core/tests/budgets.rs`, 7 new UI tests in `src/budgets.test.ts`. Not checked by launching the app, to keep test data out of the real app data folder.
