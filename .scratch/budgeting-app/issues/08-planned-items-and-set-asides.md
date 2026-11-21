# 08: Planned items and Set-asides

**What to build:** The user creates Planned items: Income or Expenses expected on a schedule, such as salary every month on the 27th, rent on the 1st, or insurance every year in June. Expected occurrences are shown on a timeline and included in Spendable. Yearly or irregular Planned items are spread as Set-asides: the yearly total divided by 12 is subtracted from every Period's Spendable, and the item's own occurrence isn't subtracted again in the month it falls due.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] The user can create, edit and end a Planned item: Account, Category (which decides Income or Expense), description, amount, schedule (monthly on a day, yearly on a date, every N months), start date and optional end date
- [ ] Planned items that occur less often than monthly default to being spread as Set-asides. The user can turn this off per item.
- [ ] The current Period's view shows its expected occurrences, along with the Set-asides
- [ ] Spendable now also includes the expected Income and Expense occurrences in the Period that aren't spread, minus the Set-asides for the Period
- [ ] Expected occurrences are calculated from the schedule and not stored
- [ ] Unit tests: a monthly salary on the 27th, a yearly 600 item giving 50 per Period, the June Period not counting the yearly item twice, and an item that ends partway through the year
