import { describe, expect, it } from "vitest";
import type { Category, TransactionView } from "./api";
import { ALL_TRANSACTIONS, filterTransactions, isFiltered, sortTransactions } from "./transactionTable";

const cat = (id: string, name: string, parent_id: string | null = null): Category => ({
  id,
  name,
  kind: "expense",
  parent_id,
  color: null,
  position: 1,
});

const categories = [
  cat("food", "Food"),
  cat("groceries", "Groceries", "food"),
  cat("restaurants", "Restaurants", "food"),
  cat("home", "Home"),
];

const txn = (id: string, fields: Partial<TransactionView> = {}): TransactionView => ({
  id,
  account_id: "bank",
  date: "2026-11-01",
  amount_minor: -1000,
  description: id,
  category_id: null,
  note: null,
  source: "import",
  import_id: null,
  fingerprint: null,
  balance_after_minor: null,
  category_source: null,
  account_name: "Bank",
  category_name: categories.find((c) => c.id === fields.category_id)?.name ?? null,
  category_kind: fields.category_id ? "expense" : null,
  ...fields,
});

const ids = (ts: TransactionView[]) => ts.map((t) => t.id);

describe("filterTransactions", () => {
  it("text search matches the description or the note, ignoring case", () => {
    const list = [
      txn("a", { description: "ESSELUNGA MILANO" }),
      txn("b", { description: "Rent", note: "shared with esselunga trip" }),
      txn("c", { description: "Cinema" }),
    ];
    expect(ids(filterTransactions(list, { ...ALL_TRANSACTIONS, text: "Esselunga" }, categories))).toEqual(["a", "b"]);
  });

  const categorised = [
    txn("food", { category_id: "food" }),
    txn("groceries", { category_id: "groceries" }),
    txn("restaurants", { category_id: "restaurants" }),
    txn("home", { category_id: "home" }),
    txn("none"),
  ];

  it("a parent Category includes its children", () => {
    const filter = { ...ALL_TRANSACTIONS, category: { categoryId: "food" } };
    expect(ids(filterTransactions(categorised, filter, categories))).toEqual(["food", "groceries", "restaurants"]);
  });

  it("a child Category does not include its parent or siblings", () => {
    const filter = { ...ALL_TRANSACTIONS, category: { categoryId: "groceries" } };
    expect(ids(filterTransactions(categorised, filter, categories))).toEqual(["groceries"]);
  });

  it("Uncategorised is a filter choice", () => {
    const filter = { ...ALL_TRANSACTIONS, category: "uncategorised" as const };
    expect(ids(filterTransactions(categorised, filter, categories))).toEqual(["none"]);
  });

  it("a Period keeps only the Transactions dated in that calendar month", () => {
    const list = [
      txn("oct31", { date: "2026-10-31" }),
      txn("nov1", { date: "2026-11-01" }),
      txn("nov30", { date: "2026-11-30" }),
      txn("dec1", { date: "2026-12-01" }),
    ];
    const filter = { ...ALL_TRANSACTIONS, period: "2026-11" };
    expect(ids(filterTransactions(list, filter, categories))).toEqual(["nov1", "nov30"]);
  });

  it("combines text, Category and Period", () => {
    const list = [
      txn("match", { date: "2026-11-05", category_id: "groceries", description: "Esselunga" }),
      txn("wrong-period", { date: "2026-10-05", category_id: "groceries", description: "Esselunga" }),
      txn("wrong-category", { date: "2026-11-05", category_id: "home", description: "Esselunga" }),
      txn("wrong-text", { date: "2026-11-05", category_id: "groceries", description: "Lidl" }),
    ];
    const filter = { text: "esse", category: { categoryId: "food" }, period: "2026-11" };
    expect(ids(filterTransactions(list, filter, categories))).toEqual(["match"]);
  });
});

describe("sortTransactions", () => {
  // As transactions_list returns them: newest first, and the later-entered first within a day.
  const list = [
    txn("nov2-late", { date: "2026-11-02", amount_minor: -300, description: "beta" }),
    txn("nov2-early", { date: "2026-11-02", amount_minor: 5000, description: "Alpha" }),
    txn("nov1", { date: "2026-11-01", amount_minor: -1200, description: "gamma" }),
  ];

  it("by date ascending puts the oldest day first, but keeps rows on the same date newest first", () => {
    expect(ids(sortTransactions(list, { key: "date", dir: "asc" }))).toEqual(["nov1", "nov2-late", "nov2-early"]);
  });

  it("by date descending is the original newest-first order", () => {
    expect(ids(sortTransactions(list, { key: "date", dir: "desc" }))).toEqual(["nov2-late", "nov2-early", "nov1"]);
  });

  it("by amount compares signed amounts", () => {
    expect(ids(sortTransactions(list, { key: "amount", dir: "asc" }))).toEqual(["nov1", "nov2-late", "nov2-early"]);
    expect(ids(sortTransactions(list, { key: "amount", dir: "desc" }))).toEqual(["nov2-early", "nov2-late", "nov1"]);
  });

  it("by description ignores case", () => {
    expect(ids(sortTransactions(list, { key: "description", dir: "asc" }))).toEqual(["nov2-early", "nov2-late", "nov1"]);
  });

  it("by Category puts Uncategorised last when ascending and first when descending", () => {
    const mixed = [
      txn("none"),
      txn("home", { category_id: "home" }),
      txn("groceries", { category_id: "groceries" }),
    ];
    expect(ids(sortTransactions(mixed, { key: "category", dir: "asc" }))).toEqual(["groceries", "home", "none"]);
    expect(ids(sortTransactions(mixed, { key: "category", dir: "desc" }))).toEqual(["none", "home", "groceries"]);
  });

  it("does not change the list it is given", () => {
    const copy = [...list];
    sortTransactions(list, { key: "amount", dir: "asc" });
    expect(list).toEqual(copy);
  });
});

describe("isFiltered", () => {
  it("is false for no filter, including blank search text", () => {
    expect(isFiltered(ALL_TRANSACTIONS)).toBe(false);
    expect(isFiltered({ ...ALL_TRANSACTIONS, text: "  " })).toBe(false);
  });

  it.each([
    ["text", { text: "rent" }],
    ["Category", { category: "uncategorised" as const }],
    ["Period", { period: "2026-11" }],
  ])("is true when a %s filter is set", (_, part) => {
    expect(isFiltered({ ...ALL_TRANSACTIONS, ...part })).toBe(true);
  });
});
