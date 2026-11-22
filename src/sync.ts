// Pure helpers for the Sync UI: summaries, the pairing countdown, and which
// views to refresh after a Sync received changes.
import type { SyncProgress, SyncReport, SyncStatus } from "./api";

/** The views of App.tsx that show synced data. */
// Home refreshes itself on `data://changed`, so it is not listed here.
export type DataView = "accounts" | "transactions" | "uncategorised" | "categories" | "budgets" | "import" | "planned" | "rules";

const ALL_VIEWS: DataView[] = ["accounts", "transactions", "uncategorised", "categories", "budgets", "import", "planned", "rules"];

/** Which views show each synced entity (by `op.entity`, the table name). */
const VIEWS_BY_ENTITY: Record<string, DataView[]> = {
  // Account names and balances appear in every list.
  account: ["accounts", "transactions", "uncategorised", "import", "planned", "rules"],
  category: ["transactions", "uncategorised", "categories", "budgets", "planned", "rules"],
  txn: ["accounts", "transactions", "uncategorised", "budgets"],
  budget: ["budgets"],
  planned_item: ["planned"],
  rule: ["rules"],
  import_profile: ["import"],
  import_batch: ["import"],
  // The Currency is reloaded by App itself.
  setting: [],
};

/** The views to refetch after a Sync touched `entities`, in nav order. */
export function viewsToRefresh(entities: string[]): DataView[] {
  const views = new Set<DataView>();
  for (const entity of entities) for (const view of VIEWS_BY_ENTITY[entity] ?? ALL_VIEWS) views.add(view);
  return ALL_VIEWS.filter((v) => views.has(v));
}

const changes = (n: number) => (n === 0 ? "none" : n === 1 ? "1 change" : `${n} changes`);

/** One line on what a finished Sync did. */
export function syncSummary(report: SyncReport): string {
  if (report.sent === 0 && report.received === 0) return "Already up to date.";
  return `Sent ${changes(report.sent)}, received ${changes(report.received)}.`;
}

export function progressText(progress: SyncProgress): string {
  return `Syncing… sending ${progress.sent}, received ${progress.received} so far`;
}

/** "m:ss" left before a pairing code expires (`expiresAt` in Unix seconds), or null once expired. */
export function expiresIn(expiresAt: number, nowMs: number): string | null {
  const left = Math.ceil(expiresAt - nowMs / 1000);
  if (left <= 0) return null;
  return `${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`;
}

/**
 * Whether turning Sync on (`enabling`) should first explain the Windows Firewall rule and ask to add it:
 * on Windows, when the rule is missing and the user hasn't answered before.
 */
export function needsFirewallExplanation(status: SyncStatus, enabling: boolean): boolean {
  return enabling && status.firewall === "missing" && !status.firewall_asked;
}
