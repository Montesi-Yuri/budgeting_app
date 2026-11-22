# 07: Matching Manual Transactions on Import

**What to build:** When an Import brings in a Transaction that the user already entered by hand as a Manual Transaction, the app suggests linking them in the review list. While the Suggestion is pending or confirmed, only the imported Transaction counts. When it's confirmed, the two merge: the imported amount and date win, and the Manual Transaction's Category and note are moved onto the imported one, as set by the user.

**Blocked by:** 03, 06

**Status:** resolved

- [x] After an Import, the app suggests matches: a Manual Transaction in the same Account, with the same amount (±1 minor unit) and a date within 4 days, that isn't already matched
- [x] Each match appears in the review list from ticket 06, with both versions side by side
- [x] While a match is pending or confirmed, the Manual Transaction is excluded from balances, Spendable and every other number
- [x] Confirming copies the Category (as user-set) and the note to the imported Transaction, and soft-deletes the Manual Transaction
- [x] Rejecting keeps both Transactions, and that pair is never suggested again
- [x] A match created on two Devices for the same pair merges into one item after Sync, because its id is derived from the pair
- [x] Unit tests for the matching window and for the merge result

## Comments

**Implemented (2026-11-22).**

- Migration `core/migrations/0014_manual_matches.sql` (appended at the end of `MIGRATIONS`): synced table `manual_match` (sync-design §1.3, with `updated_hlc`/`deleted`, no foreign keys), registered in `SYNCED_ENTITIES`.
- `budget_core::manual_matches`:
  - `ManualMatch { id, manual_txn_id, imported_txn_id, status: LinkStatus }`; `match_id(manual, imported)` = UUIDv5 of the pair, so the same match made on two Devices is one row.
  - Pure `could_match(manual, imported)`: a Manual Transaction and an imported one on the same Account, same sign, amounts within `AMOUNT_TOLERANCE_MINOR` (1), dates at most `MATCH_WINDOW_DAYS` (4) apart. Pure `merged(manual, imported)`: the imported row with the Manual Transaction's Category (as `user`) and note; a Manual Transaction without a Category/note leaves the imported one's (e.g. a Rule's Category) in place.
  - `after_import(writer, new_txns)` (crate-private), called in `imports::import_file` right after `transfers::after_import`, inside the single write: each new Transaction gets a `suggested` match with the best live Manual Transaction that isn't already matched and was never rejected with it (closest amount, then closest date).
  - `confirm(id)`: writes the merged imported Transaction, hands over the Manual Transaction's Transfer side if it had one (`transfers::hand_over_side`, keeps status/origin; skipped if the imported one is already in a two-sided Transfer), soft-deletes the Manual Transaction. `reject(id)`: keeps both, row stays `rejected`. Only a `suggested` match can be decided.
  - `HIDDEN_MANUAL_SQL` (crate-private): Manual Transactions in a non-rejected match whose imported Transaction is live. Left out of `accounts::list` balances, `totals::for_dates` (so Spendable and Budgets), `transactions::list_uncategorised`, the Import balance check, and Transfer candidates (`transfers::other_side`). Deleting the imported Transaction makes the Manual one count again.
  - `suggestions(store)`: pending matches with both Transactions live, as `ManualMatchView { manual_txn, imported_txn }`.
- `review::ReviewItem::ManualMatchSuggestion { manual_match }` (`kind: "manual_match_suggestion"`), pending (counted in the badge), filled in `review::list`.
- Commands: `manual_match_confirm(id)`, `manual_match_reject(id)`.
- UI: a "Entered by hand, then imported" card on the To review page, with a table (Entered by hand / Imported / After merging) and Merge / Keep both. `src/review.ts` `matchRows`; `src/sync.ts` entity `manual_match` refreshes review, accounts, transactions, uncategorised, budgets.
- For ticket 09/11: use `manual_matches::HIDDEN_MANUAL_SQL` (like `transfers::IN_TRANSFER_SQL`) for any new sum over `txn` (Forecast balances, Planned item fulfilment candidates).
- Open: matching runs only on Import; a Manual Transaction that arrives by Sync after the Import isn't matched. A hidden Manual Transaction still shows in its Account's Transactions list (no "matched" badge yet). As with Transfers, a rejection on one Device can be overwritten by the other Device's later Suggestion of the same pair before they sync (later HLC wins); a confirm that loses to a later reject leaves the merge done. If the imported Transaction is already in a two-sided Transfer, the Manual Transaction's own Transfer is not moved and loses that side.
- Tests: 19 in `core/tests/manual_matches.rs` (matching window, merge result, suggestion in the review list, exclusion from balances/totals/Spendable/Budgets/Uncategorised/balance check, confirm, reject, never suggested again, already matched, closest wins, Transfer interaction, deleted import, Sync merge). 3 new UI tests (`src/review.test.ts`, `src/sync.test.ts`). In total 251 Rust tests and 111 UI tests. Not checked by launching the app, to keep test data out of the real app data folder.

**Merge note (with ticket 09):** confirming a match now also hands a Planned item fulfilment over from the Manual Transaction to the imported one (`fulfilment::hand_over`, called next to `transfers::hand_over_side`), so the occurrence isn't expected again. Test: `confirming_hands_a_planned_item_fulfilment_over_to_the_imported_transaction`.
