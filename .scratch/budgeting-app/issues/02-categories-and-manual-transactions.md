# 02: Categories and Manual Transactions

**What to build:** The user manages two-level Categories, each either an income or an expense Category. They can enter Manual Transactions on an Account, and the Account balance updates. Whether a Transaction is Income or Expense is decided by its Category, so a positive amount in an expense Category is a Refund. Transactions without a Category appear in an Uncategorised list.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] The user can create, rename, reorder and delete Categories. Each is income or expense, has at most two levels (a parent and its children), and a child has the same kind as its parent.
- [ ] The user can add, edit and delete a Manual Transaction: Account, date, signed amount, description, optional Category, optional note
- [ ] An Account's balance is its Opening balance plus all its Transactions after the opening date
- [ ] A Transaction list per Account, newest first, showing the Category, or "Uncategorised" when it has none
- [ ] An Uncategorised list shows every Transaction without a Category, and the user can assign Categories from it
- [ ] Setting a Category by hand marks it as set by the user (later Rules must never overwrite it)
- [ ] Unit tests: Refunds (positive amounts in an expense Category) reduce that Category's spending and are not counted as Income
- [ ] Every write goes through the sync-logged write path from ticket 01
