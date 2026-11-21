# Budgeting App

A personal, single-user app that tracks where one person's money goes and tells them how much they can still spend. It runs on a PC and a phone that keep the same data by syncing directly with each other.

## Language

### Money and where it lives

**Account**:
A bank current account, a savings account, or a credit card in which the user holds money or owes it. Cash is not an Account: a cash withdrawal counts as spent when it is taken out.
_Avoid_: Wallet, bank

**Transaction**:
A single movement of money in or out of one Account on a given date, either imported or entered by hand. A credit card purchase is a Transaction on the card Account, dated on the day of purchase.
_Avoid_: Movement, entry, operation, txn

**Transfer**:
The movement of money between two of the user's own Accounts, made of an outgoing Transaction and an incoming Transaction. Paying the credit card bill is a Transfer. A Transfer may have only one side until the other Account is imported, and it is never an Expense or Income.
_Avoid_: Internal movement, giro

**Expense**:
A Transaction in an expense Category that is not part of a Transfer. It is usually negative. A positive Expense is a **Refund** and reduces that Category's spending. A cash withdrawal is an Expense.
_Avoid_: Cost, outgoing

**Refund**:
A positive Transaction in an expense Category, such as a returned purchase or a friend paying back their share. It is not Income.
_Avoid_: Reimbursement, payback

**Income**:
A Transaction in an income Category, such as salary, that is not part of a Transfer. Whether a Transaction is Income or Expense depends on its Category, not on the sign of its amount.
_Avoid_: Revenue, incoming

**Opening balance**:
The balance of an Account on a given date, entered by the user. All later balances for that Account are calculated forward from it.
_Avoid_: Initial balance, starting amount

**Net position**:
The sum of all Account balances, with credit card debt counted as negative.
_Avoid_: Net worth, total balance

**Category**:
A label that says what an Expense or Income was for, such as Groceries, Rent or Salary. Each Category is either an income Category or an expense Category. Categories have at most two levels: a parent such as Food, and children such as Groceries and Restaurants.
_Avoid_: Tag, type

**Currency**:
The single currency in which all amounts in the app are expressed. There is only one.

### Getting data in

**Import**:
One act of loading a bank CSV file into one Account, which turns its lines into Transactions.
_Avoid_: Upload, statement load

**Import profile**:
The saved description of one bank's CSV layout for one Account: which columns hold the date, amount and description, the date format, the separator and the decimal style. The user defines it in the app the first time they import a file from that bank.
_Avoid_: Parser, template, mapping

**Manual Transaction**:
A Transaction the user enters by hand, usually on the phone at the moment of spending. When the same Transaction later arrives in an Import, the two are merged into one: the imported amount and date win, and the user's Category and note are kept.
_Avoid_: Quick entry, pending transaction

**Rule**:
A condition that is applied automatically to Transactions when they are imported. It matches on description text, and can optionally be limited to one Account and/or to one direction (money in or out). It either assigns a Category ("description contains ESSELUNGA → Groceries") or marks the Transaction as part of a Transfer. Rules are kept in an order the user can change, and the first matching Rule wins. A Category set by hand is never changed by a Rule.
_Avoid_: Filter, auto-tag

**Uncategorised**:
The state of a Transaction that has no Category yet. It still counts towards Spendable and the Forecast.

**Suggestion**:
A link the app proposes and the user confirms or rejects. There are three kinds: two Transactions forming a Transfer; an imported Transaction fulfilling a Planned item; an imported Transaction matching a Manual Transaction. A Suggestion can also be to re-categorise existing Transactions after a Rule is added or changed. Until it is decided, the numbers treat a Suggestion as if it had been confirmed, and it waits in a list for the user to review.
_Avoid_: Auto-match

### Planning

**Period**:
A calendar month, from the 1st to the last day.
_Avoid_: Cycle, pay period

**Week**:
Monday to Sunday, cut at the Period boundary, so the first and last Weeks of a Period can be shorter than seven days.

**Planned item**:
An Income or Expense the user expects to happen on a schedule, such as salary every month, rent on the 1st, or insurance once a year in June. Each occurrence is first **expected** at the amount the user entered. When the actual Transaction arrives, the occurrence becomes **fulfilled**, the actual amount replaces the planned one, and it isn't counted twice. If nothing has arrived by the end of its Period, the user marks it **skipped** or **arrives later**.
_Avoid_: Recurring, scheduled transaction, fixed cost

**Savings target**:
A fixed amount per Period that the user intends to save. It is set aside when Spendable is calculated, and is shown next to the amount actually moved by Transfers into the savings Account in that Period.
_Avoid_: Savings goal

**Set-aside**:
The monthly share of a yearly or irregular Planned item that is subtracted from each Period's Spendable, such as 600 of insurance in June becoming 50 per Period.
_Avoid_: Sinking fund, provision, accrual

**Spendable**:
The headline number: how much the user can still spend in the current Period, after Income, Planned items, Set-asides and the Savings target are taken into account. Each Period starts fresh.
_Avoid_: Free money, available, allowance

**Weekly Spendable**:
The Period's remaining Spendable, divided by the days left in the Period and multiplied by the days left in the current Week. It is recalculated daily, so overspending shrinks the following Weeks automatically.

**Budget**:
An optional spending limit for one Category per Period or per Week, such as "Groceries: 400 per Period". A Budget on a parent Category covers its children. A Budget doesn't reserve money: Spendable covers all spending, and a Budget only warns when its Category goes over.
_Avoid_: Envelope, cap

**Forecast**:
The projected balance at the end of the current Period and at the end of the year. It is shown for the Net position (the main number) and for the current account. Everyday spending is estimated from Budgets where they exist, otherwise from the average of the last 3 Periods. With less than 3 Periods of history, the Forecast is marked as low confidence.
_Avoid_: Prediction, projection

### Devices

**Device**:
One installation of the app (the PC or the phone), each holding a full copy of the data.

**Pairing**:
The one-time introduction of two Devices by scanning a QR code, after which they trust each other.

**Sync**:
An exchange between two paired Devices in which each one receives the changes it is missing.
_Avoid_: Backup, upload

**Backup**:
A password-encrypted file containing all the data. The user can export one at any time, and the PC also writes one automatically every week to a folder the user chooses.
_Avoid_: Export, snapshot
