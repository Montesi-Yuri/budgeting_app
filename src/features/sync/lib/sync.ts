// Pure helpers for the Sync UI: summaries, the pairing countdown and the
// firewall explanation.
import type { SyncProgress, SyncReport, SyncStatus } from "@/shared/api";

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
