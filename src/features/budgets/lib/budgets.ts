// Pure helpers for showing Budgets (no I/O).

export type MeterTone = "ok" | "near" | "over";

/** From this share of the limit on, a Budget is shown as close to it. */
const NEAR_SHARE = 0.9;

/**
 * How full a Budget's progress bar is (0-100) and how it is coloured. Only
 * spending above the limit is "over": reaching it exactly is not. Refunds can
 * make spending negative; the bar then stays empty.
 */
export function budgetMeter(spentMinor: number, amountMinor: number): { percent: number; tone: MeterTone } {
  const share = amountMinor > 0 ? Math.max(spentMinor, 0) / amountMinor : 1;
  const tone: MeterTone = spentMinor > amountMinor ? "over" : share >= NEAR_SHARE ? "near" : "ok";
  return { percent: Math.min(100, Math.round(share * 100)), tone };
}
