# 04: Categorisation Rules

**What to build:** The user creates Rules that assign a Category automatically during Import. A Rule matches description text, and can optionally be limited to one Account and/or one direction (money in or out). Rules are in an order the user can change, and the first matching Rule wins. When a Rule is added or changed, the app offers to re-categorise matching existing Transactions. A Category set by the user by hand is never changed by a Rule.

**Blocked by:** 03

**Status:** resolved

- [x] The user can create, edit, delete and reorder Rules: text to look for in the description (case-insensitive), optional Account, optional direction, and the Category to assign
- [x] On Import, each new Transaction without a user-set Category gets the Category of the first matching Rule, and is marked as categorised by a Rule
- [x] After a Rule is added or edited, the app lists the existing Transactions it would re-categorise and applies the change only if the user confirms. Transactions with a user-set Category are not in the list.
- [x] Reordering Rules on one Device changes only the moved Rule's position, not every Rule's
- [x] A Rule whose Category has been deleted is shown as broken and is not applied
- [x] Unit tests: first match wins, the Account and direction filters work, user-set Categories are never overwritten

## Comments

**Implemented (2026-11-22).**

- Migration `core/migrations/0010_rules.sql` (number reserved; appended at the end of `MIGRATIONS`) adds the synced table `rule` (sync-design §1.3, with `updated_hlc` and `deleted`; no foreign keys). `action` is `'categorise'` only for now; the CHECK allows just that, so ticket 06 needs a migration to add `'transfer'` (SQLite can't alter a CHECK: rebuild the table or drop the CHECK in that migration).
- `budget_core::rules` (entity `rule`, registered in `SYNCED_ENTITIES`):
  - `Rule { id, position, match_text, account_id, direction: Option<Direction>, action: RuleAction, category_id: Option<String> }`; `Direction::{In, Out}` (in = positive amount); `RuleAction::Categorise`.
  - `create(NewRule)` (placed last), `update(RuleEdit)` (keeps its place; picking a live Category repairs a broken Rule), `delete(id)`, `move_to(id, index)` (writes only the moved Rule, with a REAL position between its neighbours, like Categories), `list` -> `RuleView { rule, category_name, account_name, broken }` in the order they are tried.
  - Matching: `Rule::matches(&txn)` (case-insensitive contains, optional Account, optional direction); `applicable(conn)` = live, non-broken Rules in order; `first_match(&rules, &txn)`; `categorise(&rules, &mut txn)` applies the first match unless `category_source = 'user'` and marks it `'rule'`.
  - `recategorise_preview(rule_id)` lists the existing Transactions this Rule is the first applicable Rule for and whose Category it would change (never user-set ones; nothing for a broken Rule). `recategorise(rule_id, &txn_ids)` applies it to the confirmed ids in one change, re-checking each, so a Category set by hand in the meantime is kept.
- Import: `imports::import_file` loads `rules::applicable` once inside its write and calls `rules::categorise` on each new (not skipped) Transaction before saving it.
- A broken Rule (its Category deleted) stays, is listed with `broken = true`, and is skipped by `applicable`. A Rule whose Account filter points to a deleted Account just never matches; the UI shows "on a deleted Account".
- For ticket 06: add `RuleAction::Transfer` (+ migration), let `NewRule`/`RuleEdit` carry the action with an optional Category, make `applicable` keep transfer Rules, and handle the action where `categorise` is called in `import_file` (`first_match` is the hook that returns the winning Rule).
- Commands: `rules_list`, `rule_create`, `rule_update`, `rule_delete`, `rule_move`, `rule_recategorise_preview`, `rule_recategorise`.
- UI: new Rules page (`src/components/RulesPage.tsx`): list in order with up/down buttons, add/edit form (text, Account, direction, Category), delete, a "Broken" badge, and after saving a Rule a "Re-categorise existing Transactions?" card listing what would change, applied only on confirm. `src/rules.ts` has `ruleSummary`.
- Tests: 14 new Rust tests in `core/tests/rules.rs` (first match wins, Account/direction filters, case-insensitive, user-set Categories never overwritten on Import or on re-categorise, broken Rules, reorder writes one row, edit/delete, Sync), 3 new UI tests in `src/rules.test.ts`. Not checked by launching the app, to keep test data out of the real app data folder.
