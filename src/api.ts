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
