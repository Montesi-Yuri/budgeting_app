# 04: Categorisation Rules

**What to build:** The user creates Rules that assign a Category automatically during Import. A Rule matches description text, and can optionally be limited to one Account and/or one direction (money in or out). Rules are in an order the user can change, and the first matching Rule wins. When a Rule is added or changed, the app offers to re-categorise matching existing Transactions. A Category set by the user by hand is never changed by a Rule.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] The user can create, edit, delete and reorder Rules: text to look for in the description (case-insensitive), optional Account, optional direction, and the Category to assign
- [ ] On Import, each new Transaction without a user-set Category gets the Category of the first matching Rule, and is marked as categorised by a Rule
- [ ] After a Rule is added or edited, the app lists the existing Transactions it would re-categorise and applies the change only if the user confirms. Transactions with a user-set Category are not in the list.
- [ ] Reordering Rules on one Device changes only the moved Rule's position, not every Rule's
- [ ] A Rule whose Category has been deleted is shown as broken and is not applied
- [ ] Unit tests: first match wins, the Account and direction filters work, user-set Categories are never overwritten
