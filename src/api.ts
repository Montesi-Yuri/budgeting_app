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
