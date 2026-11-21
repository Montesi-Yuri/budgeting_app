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
  backupExport: (path: string, passphrase: string) => invoke<void>("backup_export", { path, passphrase }),
  backupRestore: (path: string, passphrase: string) =>
    invoke<RestoreReport>("backup_restore", { path, passphrase }),
  backupScheduleGet: () => invoke<BackupSchedule>("backup_schedule_get"),
  backupScheduleSet: (folder: string | null, passphrase: string | null) =>
    invoke<BackupSchedule>("backup_schedule_set", { folder, passphrase }),
};
