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

/** Per-Device Sync settings and whether the listener runs. */
export type SyncStatus = { enabled: boolean; port: number; listening: boolean; addrs: string[] };

/** "Pair new device": the text to paste on the other Device. */
export type PairingInfo = { payload: string; expires_at: number; addrs: string[]; port: number };

/** A paired Device. Times are Unix seconds. */
export type Peer = {
  device_id: string;
  name: string;
  /** null for a Device that connects to this one. */
  last_addr: string | null;
  paired_at: number | null;
  last_sync_at: number | null;
};

export type SyncReport = {
  peer_device_id: string;
  sent: number;
  received: number;
  /** Synced entities (table names) the received changes touched. */
  entities: string[];
  converged: boolean;
};

export type SyncProgress = { peer_device_id: string; phase: "exchanging" | "finished"; sent: number; received: number };

export type JoinResult = { peer: Peer; report: SyncReport };

/** What a failed command rejects with. */
export type CommandError = { kind: "invalid" | "not_found" | "internal" | "sync"; message: string };

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
  backupExport: (path: string, passphrase: string) => invoke<void>("backup_export", { path, passphrase }),
  backupRestore: (path: string, passphrase: string) =>
    invoke<RestoreReport>("backup_restore", { path, passphrase }),
  backupScheduleGet: () => invoke<BackupSchedule>("backup_schedule_get"),
  backupScheduleSet: (folder: string | null, passphrase: string | null) =>
    invoke<BackupSchedule>("backup_schedule_set", { folder, passphrase }),
  syncStatus: () => invoke<SyncStatus>("sync_status"),
  syncServerSet: (enabled: boolean, port: number) => invoke<SyncStatus>("sync_server_set", { enabled, port }),
  pairingStart: () => invoke<PairingInfo>("pairing_start"),
  pairingCancel: () => invoke<void>("pairing_cancel"),
  /** Pair with the Device whose payload was pasted, then run the first Sync. */
  pairingJoin: (payload: string) => invoke<JoinResult>("pairing_join", { payload }),
  peersList: () => invoke<Peer[]>("peers_list"),
  peerRemove: (deviceId: string) => invoke<void>("peer_remove", { deviceId }),
  syncNow: (deviceId: string) => invoke<SyncReport>("sync_now", { deviceId }),
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
