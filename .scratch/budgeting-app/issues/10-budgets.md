# 10: Budgets

**What to build:** The user sets optional Budgets: a spending limit for a Category, per Period or per Week. A Budget on a parent Category covers its children. Budgets show progress and warn when a Category goes over, but they don't reserve money: Spendable still covers all spending.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] The user can create, edit and end a Budget on an expense Category: an amount, per Period or per Week, and the date it starts applying
- [ ] A Budget view shows, for the current Period or Week, the amount spent and the amount left for each Budget. Refunds reduce the amount spent. Transfers are not included.
- [ ] A Budget on a parent Category counts the spending of all its children
- [ ] A Budget that is over its limit is clearly flagged
- [ ] Spendable is not affected by Budgets
- [ ] A Budget whose Category is deleted is shown as broken
- [ ] Unit tests for parent/child totals and for weekly Budgets in a Week cut at the Period boundary
