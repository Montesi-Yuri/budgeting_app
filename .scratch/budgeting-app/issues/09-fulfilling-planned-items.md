# 09: Fulfilling Planned items

**What to build:** When the actual Transaction for a Planned item arrives in an Import, the app suggests it as fulfilling that occurrence, in the review list. Once it's fulfilled, the actual amount replaces the planned one and nothing counts twice. At the end of a Period, any occurrence that is still expected asks the user to mark it either skipped or arrives later (with a new expected date). It's never dropped silently.

**Blocked by:** 03, 06, 08

**Status:** ready-for-agent

- [ ] After an Import, the app suggests fulfilment: an expected occurrence in the same Account with the same sign, an amount within ±20% and a date within 5 days
- [ ] The Suggestion appears in the review list. While it's pending or confirmed, the occurrence counts as fulfilled, using the actual amount.
- [ ] The user can also link a Transaction to an occurrence by hand, or unlink it
- [ ] Occurrence states (fulfilled, skipped, arrives later) are stored with an id derived from the Planned item and the due date, so the same occurrence changed on two Devices merges into one row
- [ ] After a Period ends, its occurrences that are still expected appear in the review list asking "skipped or arrives later?". Arrives later asks for a new date, and the occurrence moves there.
- [ ] Spendable never counts both an expected occurrence and the Transaction that fulfils it
- [ ] Unit tests: a salary arriving 2 days late with a slightly different amount, a skipped occurrence, and an occurrence that arrives later in the next Period
