// Typed wrappers around the Rust commands (src-tauri/src/commands.rs).
// Types mirror budget_core; amounts are integer minor units of the Currency.
import { invoke } from "@tauri-apps/api/core";

export type AccountKind = "current" | "savings" | "credit_card";

export const ACCOUNT_KINDS: { kind: AccountKind; label: string }[] = [
  { kind: "current", label: "Current account" },
  { kind: "savings", label: "Savings account" },
  { kind: "credit_card", label: "Credit card" },
];

export type NewAccount = {
  name: string;
  kind: AccountKind;
  /** Opening balance in minor units; credit card debt is negative. */
  opening_minor: number;
  /** YYYY-MM-DD */
  opening_date: string;
  is_main: boolean;
};

export type Account = NewAccount & { id: string };

export type AccountWithBalance = Account & { balance_minor: number };

export type CategoryKind = "income" | "expense";

export type NewCategory = {
  name: string;
  kind: CategoryKind;
  /** Create it as a child of this top-level Category. */
  parent_id: string | null;
};

export type Category = {
  id: string;
  name: string;
  kind: CategoryKind;
  parent_id: string | null;
  color: string | null;
  /** Display order among siblings. */
  position: number;
};

export type NewManualTransaction = {
  account_id: string;
  /** YYYY-MM-DD */
  date: string;
  /** Signed minor units: negative = money out of the Account. */
  amount_minor: number;
  description: string;
  category_id: string | null;
  note: string | null;
};

export type TransactionEdit = NewManualTransaction & { id: string };

export type Transaction = TransactionEdit & {
  source: "import" | "manual";
  import_id: string | null;
  fingerprint: string | null;
  balance_after_minor: number | null;
  /** Who set the Category; Rules never overwrite "user". */
  category_source: "user" | "rule" | null;
};

/** A Transaction in a list. A null category_name means Uncategorised. */
export type TransactionView = Transaction & {
  account_name: string;
  category_name: string | null;
  category_kind: CategoryKind | null;
};

export type BudgetPer = "period" | "week";

export type NewBudget = {
  /** An expense Category; a parent covers its children. */
  category_id: string;
  per: BudgetPer;
  /** The limit per Period or Week, positive minor units. */
  amount_minor: number;
  /** YYYY-MM-DD: the first day it applies. */
  valid_from: string;
};

export type BudgetEdit = NewBudget & { id: string };

/** valid_to: the last day it applies, null until it is ended. */
export type Budget = BudgetEdit & { valid_to: string | null };

/** broken: its Category was deleted; it is not applied until another is picked. */
export type BudgetState = "active" | "upcoming" | "ended" | "broken";

export type BudgetProgress = {
  budget: Budget;
  state: BudgetState;
  /** The current Period or Week; null unless active. */
  range: { from: string; to: string } | null;
  /** Spending in range, reduced by Refunds. */
  spent_minor: number;
  /** Negative when over. */
  left_minor: number;
  over: boolean;
};

export type AppInfo = { device_id: string; schema_version: number };

/** What a failed command rejects with. */
export type CommandError = { kind: "invalid" | "not_found" | "internal"; message: string };

export function errorMessage(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) return String((err as CommandError).message);
  return String(err);
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  currencyGet: () => invoke<string | null>("currency_get"),
  currencySet: (code: string) => invoke<void>("currency_set", { code }),
  accountsList: () => invoke<AccountWithBalance[]>("accounts_list"),
  accountCreate: (account: NewAccount) => invoke<Account>("account_create", { account }),
  accountUpdate: (account: Account) => invoke<Account>("account_update", { account }),
  accountDelete: (id: string) => invoke<void>("account_delete", { id }),
  categoriesList: () => invoke<Category[]>("categories_list"),
  categoryCreate: (category: NewCategory) => invoke<Category>("category_create", { category }),
  categoryRename: (id: string, name: string) => invoke<Category>("category_rename", { id, name }),
  /** Move to `index` among its siblings. */
  categoryMove: (id: string, index: number) => invoke<Category>("category_move", { id, index }),
  categoryDelete: (id: string) => invoke<void>("category_delete", { id }),
  transactionsList: (accountId: string) => invoke<TransactionView[]>("transactions_list", { account: accountId }),
  transactionsUncategorised: () => invoke<TransactionView[]>("transactions_uncategorised"),
  transactionCreate: (transaction: NewManualTransaction) =>
    invoke<Transaction>("transaction_create", { transaction }),
  transactionUpdate: (transaction: TransactionEdit) => invoke<Transaction>("transaction_update", { transaction }),
  /** Set (or clear, with null) a Category by hand. */
  transactionSetCategory: (id: string, categoryId: string | null) =>
    invoke<Transaction>("transaction_set_category", { id, category: categoryId }),
  transactionDelete: (id: string) => invoke<void>("transaction_delete", { id }),
  /** Every Budget with its progress on `today` (YYYY-MM-DD). */
  budgetsProgress: (today: string) => invoke<BudgetProgress[]>("budgets_progress", { today }),
  budgetCreate: (budget: NewBudget) => invoke<Budget>("budget_create", { budget }),
  budgetUpdate: (budget: BudgetEdit) => invoke<Budget>("budget_update", { budget }),
  /** lastDay: the last day the Budget applies. */
  budgetEnd: (id: string, lastDay: string) => invoke<Budget>("budget_end", { id, lastDay }),
  budgetDelete: (id: string) => invoke<void>("budget_delete", { id }),
};

/** The Weekly Spendable for today's Week (cut at the Period boundary). */
export type WeeklySpendable = {
  week_first_day: string;
  week_last_day: string;
  /** Counting today. */
  days_left_in_period: number;
  /** Counting today. */
  days_left_in_week: number;
  amount_minor: number;
};

/** Spendable for one Period (YYYY-MM) and the terms it is made of. */
export type PeriodSpendable = {
  period: string;
  first_day: string;
  last_day: string;
  income_minor: number;
  /** Negative; Refunds included. */
  expenses_minor: number;
  /** Counted as Expenses (negative) or Refunds (positive). */
  uncategorised_minor: number;
  /** Expected Income occurrences of Planned items not spread as Set-asides. */
  expected_income_minor: number;
  /** Expected Expense occurrences of Planned items not spread as Set-asides (negative). */
  expected_expenses_minor: number;
  /** The Period's Set-asides, negative for Expenses. */
  set_asides_minor: number;
  savings_target_minor: number;
  spendable_minor: number;
  /** Only when today is in this Period. */
  weekly: WeeklySpendable | null;
};

export const spendableApi = {
  savingsTargetGet: () => invoke<number>("savings_target_get"),
  savingsTargetSet: (minor: number) => invoke<void>("savings_target_set", { minor }),
  /** `today` is the local date (YYYY-MM-DD). */
  spendableGet: (period: string, today: string) => invoke<PeriodSpendable>("spendable_get", { period, today }),
};

/**
 * When a Planned item is expected. A day past the end of a short month falls
 * on its last day. every_months counts from the month of the start date.
 */
export type Schedule =
  | { kind: "monthly"; day: number; every_months: number }
  | { kind: "yearly"; month: number; day: number };

export type PlannedItemFields = {
  account_id: string;
  /** Its kind says Income or Expense. */
  category_id: string;
  description: string;
  /** Signed: Income positive, an Expense negative. */
  amount_minor: number;
  schedule: Schedule;
  /** YYYY-MM-DD */
  start_date: string;
  /** YYYY-MM-DD, the last day it can occur; null = no end. */
  end_date: string | null;
};

/** set_aside null = the default (spread when less often than monthly). */
export type NewPlannedItem = PlannedItemFields & { set_aside: boolean | null };

export type PlannedItem = PlannedItemFields & { id: string; set_aside: boolean };

/** An expected occurrence, calculated from the schedule (never stored). */
export type Occurrence = {
  planned_item_id: string;
  description: string;
  account_id: string;
  category_id: string;
  kind: CategoryKind;
  due_date: string;
  amount_minor: number;
  /** The item is spread as Set-asides, so this occurrence isn't counted again. */
  set_aside: boolean;
};

export type SetAside = {
  planned_item_id: string;
  description: string;
  category_id: string;
  schedule: Schedule;
  item_amount_minor: number;
  /** This Period's share, negative for Expenses. */
  amount_minor: number;
};

export type PeriodPlan = { period: string; occurrences: Occurrence[]; set_asides: SetAside[] };

export const plannedApi = {
  list: () => invoke<PlannedItem[]>("planned_items_list"),
  create: (item: NewPlannedItem) => invoke<PlannedItem>("planned_item_create", { item }),
  update: (item: PlannedItem) => invoke<PlannedItem>("planned_item_update", { item }),
  /** lastDay: the last day the item can occur. */
  end: (id: string, lastDay: string) => invoke<PlannedItem>("planned_item_end", { id, lastDay }),
  delete: (id: string) => invoke<void>("planned_item_delete", { id }),
  /** The expected occurrences and Set-asides of a Period (YYYY-MM). */
  periodGet: (period: string) => invoke<PeriodPlan>("planned_period_get", { period }),
};
