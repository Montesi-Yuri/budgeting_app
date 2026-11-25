// Pure filtering and sorting for the Transactions table (no I/O).
import type { Category, TransactionView } from "./api";
import { periodOf } from "./periods";

/** Every Category, only Uncategorised Transactions, or one Category with its descendants. */
export type CategoryFilter = "all" | "uncategorised" | { categoryId: string };

export type TransactionFilter = {
  /** Matched against the description and the note, ignoring case. */
  text: string;
  category: CategoryFilter;
  /** A Period (YYYY-MM), or null for all Periods. */
  period: string | null;
};

export const ALL_TRANSACTIONS: TransactionFilter = { text: "", category: "all", period: null };

/** The Transactions that match every part of the filter, in their original order. */
export function filterTransactions(
  transactions: TransactionView[],
  filter: TransactionFilter,
  categories: Category[],
): TransactionView[] {
  const text = filter.text.trim().toLocaleLowerCase();
  const inCategory = categoryMatcher(filter.category, categories);
  return transactions.filter(
    (t) =>
      (filter.period === null || periodOf(t.date) === filter.period) &&
      inCategory(t.category_id) &&
      (text === "" ||
        t.description.toLocaleLowerCase().includes(text) ||
        (t.note ?? "").toLocaleLowerCase().includes(text)),
  );
}

/** Whether any part of the filter narrows the list. */
export function isFiltered(filter: TransactionFilter): boolean {
  return filter.text.trim() !== "" || filter.category !== "all" || filter.period !== null;
}

function categoryMatcher(filter: CategoryFilter, categories: Category[]): (id: string | null) => boolean {
  if (filter === "all") return () => true;
  if (filter === "uncategorised") return (id) => id === null;
  // Like a Budget, a parent covers its children (Categories have at most two levels).
  const covered = new Set([filter.categoryId]);
  for (const c of categories) if (c.parent_id === filter.categoryId) covered.add(c.id);
  return (id) => id !== null && covered.has(id);
}

export type SortKey = "date" | "description" | "category" | "amount";
export type TransactionSort = { key: SortKey; dir: "asc" | "desc" };

/** The order transactions_list returns: newest first. */
export const NEWEST_FIRST: TransactionSort = { key: "date", dir: "desc" };

/**
 * A sorted copy. Ties keep the order of `transactions` (newest first from
 * transactions_list), so rows on the same date stay newest first either way.
 */
export function sortTransactions(transactions: TransactionView[], sort: TransactionSort): TransactionView[] {
  const sign = sort.dir === "asc" ? 1 : -1;
  const compare = COMPARE[sort.key];
  return transactions
    .map((t, index) => ({ t, index }))
    .sort((a, b) => sign * compare(a.t, b.t) || a.index - b.index)
    .map(({ t }) => t);
}

const compareText = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });

const COMPARE: Record<SortKey, (a: TransactionView, b: TransactionView) => number> = {
  date: (a, b) => (a.date < b.date ? -1 : a.date > b.date ? 1 : 0),
  description: (a, b) => compareText(a.description, b.description),
  amount: (a, b) => a.amount_minor - b.amount_minor,
  // Uncategorised sorts after every Category.
  category: (a, b) =>
    a.category_name === null ? (b.category_name === null ? 0 : 1) : b.category_name === null ? -1 : compareText(a.category_name, b.category_name),
};
