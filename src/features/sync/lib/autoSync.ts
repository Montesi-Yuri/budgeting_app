// The phone's automatic Sync (ticket 16): when the app comes to the
// foreground it tries a quick Sync with the PC (the `sync_auto` command,
// about 1.5 s) and says nothing if the PC can't be reached.
import type { Peer } from "@/shared/api";

/** Coming back to the app sooner than this after an attempt doesn't try again. */
export const AUTO_SYNC_MIN_GAP_MS = 15_000;

export type AutoSyncState = {
  /** A Sync (automatic or "Sync now") is running. */
  syncing: boolean;
  /** When the last automatic Sync was tried (ms), if ever. */
  lastAttemptMs: number | null;
};

/** Whether coming to the foreground should start an automatic Sync with `target`. */
export function shouldAutoSync(target: Peer | null, state: AutoSyncState, nowMs: number): boolean {
  if (!target || state.syncing) return false;
  return state.lastAttemptMs === null || nowMs - state.lastAttemptMs >= AUTO_SYNC_MIN_GAP_MS;
}

const plural = (n: number, unit: string) => `${n} ${unit}${n === 1 ? "" : "s"} ago`;

/** When the last successful Sync happened (`lastSyncAt` in Unix seconds), for the phone's Home. */
export function lastSyncText(lastSyncAt: number | null, nowMs: number): string {
  if (lastSyncAt === null) return "never";
  const secs = Math.max(0, Math.floor(nowMs / 1000 - lastSyncAt));
  if (secs < 60) return "just now";
  if (secs < 60 * 60) return plural(Math.floor(secs / 60), "minute");
  if (secs < 24 * 60 * 60) return plural(Math.floor(secs / 3600), "hour");
  return new Date(lastSyncAt * 1000).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}
