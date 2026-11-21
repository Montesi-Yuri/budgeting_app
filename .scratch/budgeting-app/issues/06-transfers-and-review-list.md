# 06: Transfers and the review list

**What to build:** Money moved between the user's own Accounts is recognised as a Transfer, and is never counted as Income or Expense. Paying the credit card bill is a Transfer. Rules can mark Transactions as part of a Transfer, and the app also suggests likely pairs. This ticket introduces the review list: a "to review" badge and screen where the user confirms or rejects Suggestions. Until the user decides, a Suggestion counts as if it were confirmed. A Transfer can have only one side until the other Account is imported. The home screen also shows savings progress: the Savings target next to the amount actually transferred into savings Accounts this Period.

**Blocked by:** 04, 05

**Status:** ready-for-agent

- [ ] A Rule can have the action "mark as Transfer" instead of a Category. Transactions it matches become a confirmed Transfer straight away (one side only, if the other side is missing).
- [ ] After an Import, the app suggests Transfers: same absolute amount, opposite signs, different Accounts, dates within 3 days, and neither side already part of a Transfer. A Transfer with only one side is completed when its matching other side arrives.
- [ ] A Suggestion created for the same pair on two Devices merges into one item after Sync, because its id is derived from the pair
- [ ] The review list shows pending Suggestions with a count badge. Confirm and reject act on one item. A rejected pair is never suggested again.
- [ ] The user can also link two Transactions as a Transfer by hand, or unlink one
- [ ] Transactions in a Transfer that hasn't been rejected are excluded from Income, Expenses and Spendable
- [ ] A Transfer that still has one side at the end of its Period stays a Transfer and is shown as "other side not imported yet"
- [ ] The home screen shows the Savings target next to the total transferred into savings Accounts in the current Period
- [ ] Unit tests for the matching window and for excluding Transfers from Spendable
