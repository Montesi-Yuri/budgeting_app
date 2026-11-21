# 01: App skeleton with Accounts

**What to build:** The Tauri + React app runs on the PC. The user can create, edit and delete Accounts (current, savings, credit card), each with an Opening balance on a date, and sees every Account's current balance. From this first change onwards, every write records a change in the sync log, stamped with the hybrid logical clock, as described in the sync design. The single Currency is a synced setting.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The app starts on Windows and opens a local SQLite database, which is created with migrations on first launch
- [ ] On first launch the Device gets a permanent device id
- [ ] The user can create, rename and delete an Account of kind current, savings or credit card, with an Opening balance and its date. One Account can be marked as the main (current) Account.
- [ ] Each Account shows its balance (the Opening balance for now). Credit card debt is shown as negative.
- [ ] The Currency is set once in settings and used to format every amount. Amounts are stored as integer minor units.
- [ ] Every create, edit and delete goes through a single write path. It updates the row and appends a change to the sync log in the same DB transaction. Deletes are soft deletes.
- [ ] The hybrid logical clock has unit tests: it only moves forward, including when the wall clock goes backwards, and ties are broken by device id
- [ ] The glossary terms are used in the UI and code (Account, Opening balance, Currency)
