# 05: Spendable (basic)

**What to build:** A home screen shows Spendable for the current Period (calendar month) and the Weekly Spendable. In this first version Spendable is: Income in the Period, plus Expenses in the Period (negative, with Refunds included), minus the Savings target. The Savings target is a fixed amount per Period, set in settings. Weekly Spendable is the Period's remaining Spendable, divided by the days left in the Period and multiplied by the days left in the current Week. Weeks run Monday to Sunday and are cut at the Period boundary.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] The Savings target can be set in settings and is synced
- [ ] The home screen shows Spendable for the current Period, and the Weekly Spendable for the current Week
- [ ] Spendable and Weekly Spendable are recalculated whenever data changes
- [ ] Each Period starts fresh: overspending in one Period does not carry into the next Period's Spendable
- [ ] Uncategorised Transactions count towards Spendable. Without a Category they can't be classified as Income, so they are treated as Expenses (negative) or Refunds (positive).
- [ ] The user can look at past Periods' Spendable
- [ ] Unit tests for the calculation, including: a Week that crosses a Period boundary, overspending early in the Period (later Weeks shrink), underspending (later Weeks grow), and the last day of the Period
