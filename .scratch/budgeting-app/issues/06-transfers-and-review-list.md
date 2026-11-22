# 06: Transfers and the review list

**What to build:** Money moved between the user's own Accounts is recognised as a Transfer, and is never counted as Income or Expense. Paying the credit card bill is a Transfer. Rules can mark Transactions as part of a Transfer, and the app also suggests likely pairs. This ticket introduces the review list: a "to review" badge and screen where the user confirms or rejects Suggestions. Until the user decides, a Suggestion counts as if it were confirmed. A Transfer can have only one side until the other Account is imported. The home screen also shows savings progress: the Savings target next to the amount actually transferred into savings Accounts this Period.

**Blocked by:** 04, 05

**Status:** resolved

- [x] A Rule can have the action "mark as Transfer" instead of a Category. Transactions it matches become a confirmed Transfer straight away (one side only, if the other side is missing).
- [x] After an Import, the app suggests Transfers: same absolute amount, opposite signs, different Accounts, dates within 3 days, and neither side already part of a Transfer. A Transfer with only one side is completed when its matching other side arrives.
- [x] A Suggestion created for the same pair on two Devices merges into one item after Sync, because its id is derived from the pair
- [x] The review list shows pending Suggestions with a count badge. Confirm and reject act on one item. A rejected pair is never suggested again.
- [x] The user can also link two Transactions as a Transfer by hand, or unlink one
- [x] Transactions in a Transfer that hasn't been rejected are excluded from Income, Expenses and Spendable
- [x] A Transfer that still has one side at the end of its Period stays a Transfer and is shown as "other side not imported yet"
- [x] The home screen shows the Savings target next to the total transferred into savings Accounts in the current Period
- [x] Unit tests for the matching window and for excluding Transfers from Spendable

## Comments

**Implemented (2026-11-22).**

- Migration `core/migrations/0012_transfers.sql` (appended at the end of `MIGRATIONS`): synced table `transfer` (sync-design §1.3, with `updated_hlc`/`deleted`, no foreign keys; `status` suggested/confirmed/rejected, `origin` rule/match/user), registered in `SYNCED_ENTITIES`. It also rebuilds `rule` so `action` may be `'transfer'` (existing rows are copied).
- `budget_core::review`: `LinkStatus` (shared by every Suggestion; tickets 07/09 reuse it), `ReviewItem` (tagged `kind`: `transfer_suggestion`, `transfer_missing_side`; 07/09 add variants), `list(store, today) -> ReviewList { items, pending_count }`. Pending Suggestions come first; "other side not imported yet" items are listed once their Period has ended and are not counted in the badge.
- `budget_core::transfers`:
  - `Transfer` row; ids `pair_id(out, in)` = UUIDv5 of the pair and `side_id(txn)` = UUIDv5 of the one side, so the same Transfer made on two Devices is one row. When the other side arrives, the one-sided row is soft-deleted and the pair row takes over its status and origin.
  - Pure `could_pair(a, b)`: same absolute amount, opposite signs, different Accounts, at most `MATCH_WINDOW_DAYS` (3) apart.
  - `after_import(writer, new_txns, marked_by_rule)` (crate-private, called by `imports::import_file` inside its single write): a Rule-marked Transaction becomes a confirmed Transfer (paired if its other side is there, else one side); any other new Transaction gets a `suggested` Transfer with the closest candidate that is in no Transfer (or completes a one-sided one) and was never rejected as that pair.
  - `link(a, b)` by hand (one out, one in, different Accounts; amounts may differ, e.g. a fee; completes a one-sided Transfer; errors if either is already in a two-sided one), `confirm(id)`, `reject(id)` (= unlink; the row is kept as `rejected`; a Rule's Transfer that was completed goes back to its one Rule side), `list` (non-rejected, with live `TransactionView` sides, oldest first), `savings_progress(period) -> SavingsProgress { target_minor, transferred_minor }` (incoming sides on savings Accounts in the Period, not from another savings Account).
  - `IN_TRANSFER_SQL` (crate-private): the ids of Transactions in a non-rejected Transfer. `totals::for_dates` and `transactions::list_uncategorised` leave them out, so Income, Expenses, Spendable and Budgets all exclude Transfers, and Transfers don't need a Category.
- Rules: `RuleAction::Transfer`; `NewRule`/`RuleEdit` have `action` (`#[serde(default)]` = categorise) and `category_id: Option<String>` (cleared for a Transfer Rule). Transfer Rules are never broken, are kept by `applicable`, leave the Category alone, and list nothing in `recategorise_preview`. The first matching Rule wins, as before.
- Commands: `review_list(today)`, `transfers_list`, `transfer_link(a, b)`, `transfer_confirm(id)`, `transfer_reject(id)`, `savings_progress_get(period)`.
- UI: a "To review" page (second in the nav, with a count badge refreshed on navigation, after a Sync and after each decision): suggested Transfers with Confirm/Reject, "Other side not imported yet", a "Link a Transfer by hand" form, and all Transfers with Unlink. The Rule form has "Then: Assign a Category / Mark as Transfer". Home has a Savings card (target next to the amount transferred into savings Accounts, with a progress bar). `src/review.ts`: `badgeText`, `sideText`, `linkCandidates`. `src/sync.ts`: new view `review`, entity `transfer` refreshes review, uncategorised and budgets.
- For ticket 07/09: add a `ReviewItem` variant and fill it in `review::list`; use `LinkStatus`; find matches in `import_file` next to `transfers::after_import` (it gets the new Transactions). For ticket 11: Transfers already drop out of `totals::for_dates`; for balances use all Transactions (a Transfer moves money between Accounts, so the Net position is unchanged only when both sides are imported).
- Open: if two Devices import the same pair before syncing and one of them rejects it before the other's Suggestion arrives, the later write wins (by HLC), so a rejected Suggestion can come back as suggested. Unlinking a Rule's two-sided Transfer that was paired at once (not completed later) rejects it entirely. Linking by hand allows different amounts.
- Tests: 20 in `core/tests/transfers.rs` (matching window, exclusion from totals/Spendable/Uncategorised, suggestions, confirm/reject, never suggested again, Rule Transfers with one side and completion, link/unlink, missing side at the end of the Period, Sync merge, savings progress); `core/tests/rules.rs` updated for the new fields. 7 new UI tests (`src/review.test.ts`, `rules.test.ts`, `sync.test.ts`). In total 229 Rust tests and 93 UI tests. Not checked by launching the app, to keep test data out of the real app data folder.

