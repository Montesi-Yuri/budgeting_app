# 02: Categories and Manual Transactions

**What to build:** The user manages two-level Categories, each either an income or an expense Category. They can enter Manual Transactions on an Account, and the Account balance updates. Whether a Transaction is Income or Expense is decided by its Category, so a positive amount in an expense Category is a Refund. Transactions without a Category appear in an Uncategorised list.

**Blocked by:** 01

**Status:** resolved

- [x] The user can create, rename, reorder and delete Categories. Each is income or expense, has at most two levels (a parent and its children), and a child has the same kind as its parent.
- [x] The user can add, edit and delete a Manual Transaction: Account, date, signed amount, description, optional Category, optional note
- [x] An Account's balance is its Opening balance plus all its Transactions after the opening date
- [x] A Transaction list per Account, newest first, showing the Category, or "Uncategorised" when it has none
- [x] An Uncategorised list shows every Transaction without a Category, and the user can assign Categories from it
- [x] Setting a Category by hand marks it as set by the user (later Rules must never overwrite it)
- [x] Unit tests: Refunds (positive amounts in an expense Category) reduce that Category's spending and are not counted as Income
- [x] Every write goes through the sync-logged write path from ticket 01

## Comments

**Implemented (2026-11-21).**

- Migration `core/migrations/0002_categories_and_transactions.sql` adds the synced tables `category` and `txn` (sync-design §1.3, with `updated_hlc` and `deleted`). References between rows are not foreign key constraints, so remote ops can apply in any order; the domain code checks them when the user writes.
- `budget_core::categories`: `create`, `list` (each parent followed by its children), `rename`, `move_to(id, index)` (writes only the moved row, with a REAL position between its neighbours) and `delete` (a parent's children are deleted in the same change). The kind is chosen at creation; a child takes its parent's kind.
- `budget_core::transactions`: `create_manual`, `update`, `set_category`, `delete`, `list_for_account` (newest first) and `list_uncategorised`. A Category chosen by hand gets `category_source = 'user'`; an edit that leaves the Category alone keeps whoever set it (so a Rule's Category stays a Rule's). Transactions keep their `category_id` when that Category is deleted (sync-design §6), and reads treat them as Uncategorised.
- `accounts::list` balance = Opening balance + live Transactions dated after the opening date.
- `budget_core::totals::for_dates(from, to)` gives Income, Expenses (Refunds included), Uncategorised and per-Category net totals, with `spending_minor(category_id)`. Ticket 05 should build Spendable on it; Transfers (06) and matched Manual Transactions (07) are not excluded yet.
- UI: new Transactions, Uncategorised and Categories pages. `src/categories.ts` holds the pure helpers (`transactionFlow` shows a Refund badge).
- Sync (ticket 13): register the entities `category` (`categories::Category`) and `txn` (`transactions::Transaction`) for `apply_remote`.
- Tests: 53 Rust tests (`npm run test:rust`) and 35 UI tests (`npm test`). Not checked by launching the app, to keep test data out of the real app data folder.
