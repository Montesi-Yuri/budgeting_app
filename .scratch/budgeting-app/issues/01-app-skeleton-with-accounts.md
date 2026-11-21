# 01: App skeleton with Accounts

**What to build:** The Tauri + React app runs on the PC. The user can create, edit and delete Accounts (current, savings, credit card), each with an Opening balance on a date, and sees every Account's current balance. From this first change onwards, every write records a change in the sync log, stamped with the hybrid logical clock, as described in the sync design. The single Currency is a synced setting.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] The app starts on Windows and opens a local SQLite database, which is created with migrations on first launch
- [x] On first launch the Device gets a permanent device id
- [x] The user can create, rename and delete an Account of kind current, savings or credit card, with an Opening balance and its date. One Account can be marked as the main (current) Account.
- [x] Each Account shows its balance (the Opening balance for now). Credit card debt is shown as negative.
- [x] The Currency is set once in settings and used to format every amount. Amounts are stored as integer minor units.
- [x] Every create, edit and delete goes through a single write path. It updates the row and appends a change to the sync log in the same DB transaction. Deletes are soft deletes.
- [x] The hybrid logical clock has unit tests: it only moves forward, including when the wall clock goes backwards, and ties are broken by device id
- [x] The glossary terms are used in the UI and code (Account, Opening balance, Currency)

## Comments

**Implemented (2026-11-21).** There is no "done" triage label, so this ticket uses `resolved` from the issue-tracker conventions.

- The core lives in `core/` (crate `budget_core`). The single write path is `Store::write(|w| w.upsert(&row) / w.delete::<Row>(id))` in `core/src/store.rs`, and that file's docs explain how to add a synced entity. The HLC is in `core/src/hlc.rs`. See ADR 0002.
- Tests: 28 Rust tests (`npm run test:rust`) and 23 UI tests (`npm test`).
- I checked by launching the debug build: the DB is created at `%APPDATA%\app.budgeting.personal\budget.sqlite3` with schema 1 and a self device row, and every command worked through the WebView.
- Not done yet: Account balances are the Opening balance only, until ticket 02 adds Transactions. A credit card debt is typed as a negative amount, and the form says so.
