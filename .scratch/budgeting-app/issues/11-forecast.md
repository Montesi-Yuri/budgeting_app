# 11: Forecast

**What to build:** A Forecast screen shows the projected balance at the end of the current Period and at the end of the year. The main number is for the Net position (all Accounts, with credit card debt negative), and the main current Account is shown as well. The projection starts from today's balance and adds the expected occurrences of Planned items. It then adds estimated everyday spending for each expense Category: the Category's Budget where one is set, otherwise the average of the last 3 Periods. With less than 3 Periods of history, the Forecast is marked as low confidence.

**Blocked by:** 06, 08, 10

**Status:** ready-for-agent

- [ ] The Forecast shows the end of the Period and the end of the year, for the Net position and for the main Account
- [ ] Expected Planned item occurrences are added on their due dates. Yearly items appear in their actual month, not as Set-asides.
- [ ] Everyday spending per expense Category comes from its Budget if one exists, otherwise from the average of the last 3 complete Periods. Categories already covered by Planned items are left out to avoid counting them twice.
- [ ] Transfers move money between Accounts but don't change the Net position
- [ ] With less than 3 complete Periods of history, the Forecast is shown as low confidence
- [ ] A simple month-by-month view of the projected Net position until the end of the year
- [ ] Unit tests: a Forecast with Budgets only, with averages only, with a yearly Planned item, and the low-confidence case
