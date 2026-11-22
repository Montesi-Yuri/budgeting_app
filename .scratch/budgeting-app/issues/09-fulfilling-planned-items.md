# 09: Fulfilling Planned items

**What to build:** When the actual Transaction for a Planned item arrives in an Import, the app suggests it as fulfilling that occurrence, in the review list. Once it's fulfilled, the actual amount replaces the planned one and nothing counts twice. At the end of a Period, any occurrence that is still expected asks the user to mark it either skipped or arrives later (with a new expected date). It's never dropped silently.

**Blocked by:** 03, 06, 08

**Status:** resolved

- [x] After an Import, the app suggests fulfilment: an expected occurrence in the same Account with the same sign, an amount within ±20% and a date within 5 days
- [x] The Suggestion appears in the review list. While it's pending or confirmed, the occurrence counts as fulfilled, using the actual amount.
- [x] The user can also link a Transaction to an occurrence by hand, or unlink it
- [x] Occurrence states (fulfilled, skipped, arrives later) are stored with an id derived from the Planned item and the due date, so the same occurrence changed on two Devices merges into one row
- [x] After a Period ends, its occurrences that are still expected appear in the review list asking "skipped or arrives later?". Arrives later asks for a new date, and the occurrence moves there.
- [x] Spendable never counts both an expected occurrence and the Transaction that fulfils it
- [x] Unit tests: a salary arriving 2 days late with a slightly different amount, a skipped occurrence, and an occurrence that arrives later in the next Period

## Comments

**Implemented (2026-11-22).**

- Migration `core/migrations/0015_planned_occurrences.sql` (appended at the end of `MIGRATIONS`): synced table `planned_occurrence` (sync-design §1.3, with `updated_hlc`/`deleted`, no foreign keys), registered in `SYNCED_ENTITIES`. `state` is fulfilled / skipped / later, plus `expected`, which only remembers a rejected fulfilment. `link_status` may be `rejected`, so a Transaction rejected for an occurrence is never suggested for it again.
- `budget_core::fulfilment`:
  - `PlannedOccurrence` row, id `occurrence_id(planned_item_id, due_date)` = UUIDv5 of the item and the schedule's date, so the same occurrence changed on two Devices is one row (the later change wins by HLC).
  - Pure `could_fulfil(occurrence, txn)`: same Account, same sign, amount within ±20% of the planned one, at most 5 days from the date it is expected on.
  - `after_import(writer, new_txns)` (crate-private), called by `imports::import_file` right after `transfers::after_import`: each new Transaction that isn't in a Transfer gets a `suggested` fulfilment of the closest expected occurrence (date, then amount).
  - `link(item, due_date, txn)` by hand (confirmed, any amount/date; a Transaction fulfils only one occurrence), `confirm(id)`, `unlink(id)` (= reject; the occurrence is expected again, on its new date if it was moved), `skip(item, due_date)`, `arrives_later(item, due_date, new_date)` (new date after the due date; it can be moved again), `list() -> Vec<Fulfilment>`.
  - A fulfilment whose Transaction is deleted leaves the occurrence expected again.
- `planned::expected_occurrences` is the single source of "still expected": it leaves out skipped and fulfilled (suggested or confirmed) occurrences and moves those that arrive later. `Occurrence` gains `id` and `scheduled_date`; `due_date` is now the date it is expected on (the moved date, if it arrives later), `scheduled_date` the schedule's date that identifies it.
- Spendable: a fulfilled occurrence is no longer expected and its Transaction counts instead, so nothing counts twice. A Transaction fulfilling an item spread as Set-asides is taken back out by a new term, `PeriodPlan::covered_by_set_asides_minor` / `PeriodSpendable::covered_by_set_asides_minor` (positive for an Expense; shown on Home as "Paid from Set-asides"). `totals::for_dates` is unchanged, so Budgets still see that spending in its Category.
- Review list: new `ReviewItem` variants `fulfilment_suggestion { fulfilment }` and `still_expected { occurrence }` (occurrences expected before today's Period), both pending (counted in the badge).
- Commands: `fulfilments_list`, `fulfilment_link`, `fulfilment_confirm`, `fulfilment_unlink`, `occurrence_skip`, `occurrence_arrives_later`. UI: `src/components/PlannedReview.tsx` on the "To review" page (Suggestions with Confirm/Reject, "Still expected: skipped or arrives later?" with a date, "Link to a Planned item by hand", fulfilled items with Unlink), `fulfilmentApi` in `api.ts`, helpers in `src/fulfilment.ts`. `src/sync.ts`: `planned_occurrence` refreshes review; `planned_item` now refreshes review too.
- For ticket 11 (Forecast): use `planned::expected_occurrences(store, from, to)` and its `due_date` (already moved for "arrives later").
- Seams tested (chosen without the user, as a subagent): `fulfilment::could_fulfil`, the `fulfilment` actions, `imports::import_file`, `review::list`, `spendable::for_period`, `planned::expected_occurrences`/`for_period`, and Sync via `apply_remote`/`sync_pair`.
- Open: a Planned item whose start date is in the past asks "skipped or arrives later?" for every past occurrence (none is dropped silently, as asked). A late occurrence fulfilled in the next Period leaves its own Period without it, and the Transaction counts in the Period it is dated in. A Manual Transaction that fulfils an occurrence and is later hidden by a match (ticket 07) keeps the fulfilment on the manual id; the numbers stay right except that a Set-aside item's payment would no longer be taken back out. As with Transfers, two Devices deciding before syncing: the later write wins.
- Tests: 17 in `core/tests/fulfilment.rs`; 4 new UI tests (`src/fulfilment.test.ts`, `src/sync.test.ts`). In total 249 Rust tests and 112 UI tests. Not checked by launching the app, to keep test data out of the real app data folder.
