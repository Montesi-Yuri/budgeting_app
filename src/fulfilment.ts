// Pure helpers for fulfilling Planned items in the review list (no I/O).
import type { Fulfilment, Occurrence, TransactionView } from "./api";

/**
 * The Transactions `occurrence` can be linked to by hand: on its Account,
 * with the same sign (any amount or date), and not already fulfilling an
 * occurrence.
 */
export function fulfilmentCandidates(
  occurrence: Occurrence,
  transactions: TransactionView[],
  fulfilments: Fulfilment[],
): TransactionView[] {
  const used = new Set(fulfilments.map((f) => f.transaction.id));
  return transactions.filter(
    (t) =>
      t.account_id === occurrence.account_id &&
      Math.sign(t.amount_minor) === Math.sign(occurrence.amount_minor) &&
      !used.has(t.id),
  );
}

/** This Period's expected occurrences and the overdue ones, each once, by date. */
export function linkableOccurrences(thisPeriod: Occurrence[], overdue: Occurrence[]): Occurrence[] {
  const byId = new Map<string, Occurrence>();
  for (const o of [...overdue, ...thisPeriod]) byId.set(o.id, o);
  return [...byId.values()].sort((a, b) => a.due_date.localeCompare(b.due_date) || a.id.localeCompare(b.id));
}
