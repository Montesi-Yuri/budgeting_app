// Typed wrappers around the Rust commands (src-tauri/src/commands.rs).
// Types mirror budget_core; amounts are integer minor units of the Currency.
import { invoke } from "@tauri-apps/api/core";
import type { RestoreReport } from "./backup";

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

export type RuleDirection = "in" | "out";

/** What the user enters for a Rule. match_text is looked for in the description, ignoring case. */
export type NewRule = {
  match_text: string;
  /** null = any Account. */
  account_id: string | null;
  /** null = money in or out. */
  direction: RuleDirection | null;
  category_id: string;
};

export type RuleEdit = NewRule & { id: string };

export type Rule = Omit<RuleEdit, "category_id"> & {
  /** First match wins, lowest first. */
  position: number;
  action: "categorise";
  category_id: string | null;
};

/** broken: its Category was deleted; it is not applied until another is picked. */
export type RuleView = Rule & { category_name: string | null; account_name: string | null; broken: boolean };

export type ImportEncoding = "utf-8" | "windows-1252";
export type AmountMode = "signed" | "split";

/** One Account's saved CSV layout. Columns are named by their header text. */
export type ImportProfile = {
  id: string;
  account_id: string;
  name: string;
  encoding: ImportEncoding;
  /** One character: ";", "," or a tab. */
  delimiter: string;
  /** Lines before the header line. */
  skip_rows: number;
  date_column: string;
  /** strftime style, e.g. "%d/%m/%Y". */
  date_format: string;
  amount_mode: AmountMode;
  amount_column: string | null;
  debit_column: string | null;
  credit_column: string | null;
  decimal_sep: "," | ".";
  thousands_sep: string | null;
  /** Joined with a space, in this order. */
  description_columns: string[];
  balance_column: string | null;
};

export type PreviewRow = {
  line: number;
  date: string | null;
  amount_minor: number | null;
  description: string;
  balance_after_minor: number | null;
  /** Why this line can't be imported. */
  error: string | null;
};

export type ImportPreview = {
  first_lines: string[];
  columns: string[];
  /** Stops the profile from reading the file at all; rows is then empty. */
  problem: string | null;
  rows: PreviewRow[];
  total_rows: number;
  rows_with_errors: number;
};

/** One Import in an Account's history. */
export type ImportBatch = {
  id: string;
  account_id: string;
  profile_id: string;
  source_name: string;
  /** Unix ms. */
  imported_at: number;
  first_date: string | null;
  last_date: string | null;
  rows_new: number;
  rows_skipped: number;
};

export type BalanceMismatch = {
  line: number;
  date: string;
  description: string;
  file_balance_minor: number;
  calculated_balance_minor: number;
};

export type ImportSummary = { batch: ImportBatch; balance_mismatch: BalanceMismatch | null };

/** The weekly automatic Backup (PC only). */
export type BackupSchedule = {
  available: boolean;
  folder: string | null;
  /** File name of the newest weekly Backup in the folder. */
  latest: string | null;
  last_error: string | null;
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
  importProfileGet: (accountId: string) => invoke<ImportProfile | null>("import_profile_get", { account: accountId }),
  importProfileDraft: (accountId: string) => invoke<ImportProfile>("import_profile_draft", { account: accountId }),
  importProfileSave: (profile: ImportProfile) => invoke<ImportProfile>("import_profile_save", { profile }),
  /** Read the file's bytes with a profile; nothing is saved. */
  importPreview: (profile: ImportProfile, file: Uint8Array) =>
    invoke<ImportPreview>("import_preview", { profile, file: Array.from(file) }),
  /** Import with the Account's saved profile. The file itself is not kept. */
  importRun: (accountId: string, fileName: string, file: Uint8Array) =>
    invoke<ImportSummary>("import_run", { account: accountId, fileName, file: Array.from(file) }),
  importsHistory: (accountId: string) => invoke<ImportBatch[]>("imports_history", { account: accountId }),
  /** Every Rule, in the order they are tried. */
  rulesList: () => invoke<RuleView[]>("rules_list"),
  ruleCreate: (rule: NewRule) => invoke<Rule>("rule_create", { rule }),
  ruleUpdate: (rule: RuleEdit) => invoke<Rule>("rule_update", { rule }),
  ruleDelete: (id: string) => invoke<void>("rule_delete", { id }),
  /** Move to `index` in the order (0 = tried first). Only this Rule is written. */
  ruleMove: (id: string, index: number) => invoke<Rule>("rule_move", { id, index }),
  /** The existing Transactions the Rule would re-categorise; nothing is changed. */
  ruleRecategorisePreview: (id: string) => invoke<TransactionView[]>("rule_recategorise_preview", { id }),
  /** Apply the Rule to the confirmed Transactions. Returns how many changed. */
  ruleRecategorise: (id: string, transactionIds: string[]) =>
    invoke<number>("rule_recategorise", { id, transactions: transactionIds }),
  backupExport: (path: string, passphrase: string) => invoke<void>("backup_export", { path, passphrase }),
  backupRestore: (path: string, passphrase: string) =>
    invoke<RestoreReport>("backup_restore", { path, passphrase }),
  backupScheduleGet: () => invoke<BackupSchedule>("backup_schedule_get"),
  backupScheduleSet: (folder: string | null, passphrase: string | null) =>
    invoke<BackupSchedule>("backup_schedule_set", { folder, passphrase }),
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
