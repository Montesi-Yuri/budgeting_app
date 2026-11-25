// Pure helpers for the phone's quick "add Manual Transaction" form (ticket 15).
import type { AccountWithBalance } from "@/shared/api";
import { parseAmount, ParsedAmount } from "@/shared/lib/money";

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
