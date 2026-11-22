// Pure helpers for the phone build (ticket 15): the quick "add Manual
// Transaction" form, the QR scanner and "Sync now".
import type { AccountWithBalance, CommandError, Peer } from "./api";
import { parseAmount, ParsedAmount } from "./money";

export type Direction = "out" | "in";

/** The quick add amount, typed without a sign: money out is stored negative. */
export function quickAmount(text: string, direction: Direction, currency: string): ParsedAmount {
  const parsed = parseAmount(text.trim().replace(/^[+-]/, ""), currency);
  if (!parsed.ok) return parsed;
  if (parsed.minor === 0) return { ok: false, error: "Enter an amount other than 0" };
  return { ok: true, minor: direction === "out" ? -parsed.minor : parsed.minor };
}

/** The Account preselected in quick add: the last used, else the main one, else the first. */
export function defaultAccountId(accounts: AccountWithBalance[], lastUsed: string | null): string | null {
  if (lastUsed && accounts.some((a) => a.id === lastUsed)) return lastUsed;
  return (accounts.find((a) => a.is_main) ?? accounts[0])?.id ?? null;
}

const PAIRING_PREFIX = "bsync:";

export type Scanned = { ok: true; code: string } | { ok: false; error: string };

/** Check what the camera read before trying to pair with it. */
export function scannedPairingCode(content: string): Scanned {
  const code = content.trim();
  if (code.startsWith(PAIRING_PREFIX)) return { ok: true, code };
  return {
    ok: false,
    error: "This QR code is not a pairing code. On the PC, open Settings → Sync → Pair new device and scan the code shown there.",
  };
}

/** A Sync failed because the PC couldn't be reached: offer to rescan its QR code. */
export function isUnreachable(err: unknown): boolean {
  return typeof err === "object" && err !== null && (err as CommandError).kind === "unreachable";
}

/** The Device "Sync now" connects to: one with a known address, the most recently synced first. */
export function syncTarget(peers: Peer[]): Peer | null {
  const reachable = peers.filter((p) => p.last_addr !== null);
  reachable.sort((a, b) => (b.last_sync_at ?? 0) - (a.last_sync_at ?? 0));
  return reachable[0] ?? null;
}
