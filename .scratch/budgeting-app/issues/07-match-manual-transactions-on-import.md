# 07: Matching Manual Transactions on Import

**What to build:** When an Import brings in a Transaction that the user already entered by hand as a Manual Transaction, the app suggests linking them in the review list. While the Suggestion is pending or confirmed, only the imported Transaction counts. When it's confirmed, the two merge: the imported amount and date win, and the Manual Transaction's Category and note are moved onto the imported one, as set by the user.

**Blocked by:** 03, 06

**Status:** ready-for-agent

- [ ] After an Import, the app suggests matches: a Manual Transaction in the same Account, with the same amount (±1 minor unit) and a date within 4 days, that isn't already matched
- [ ] Each match appears in the review list from ticket 06, with both versions side by side
- [ ] While a match is pending or confirmed, the Manual Transaction is excluded from balances, Spendable and every other number
- [ ] Confirming copies the Category (as user-set) and the note to the imported Transaction, and soft-deletes the Manual Transaction
- [ ] Rejecting keeps both Transactions, and that pair is never suggested again
- [ ] A match created on two Devices for the same pair merges into one item after Sync, because its id is derived from the pair
- [ ] Unit tests for the matching window and for the merge result
