import { describe, expect, it } from "vitest";
import type { TransactionView, TransferView } from "./api";
import { badgeText, linkCandidates, matchRows, sideText } from "./review";

const txn = (over: Partial<TransactionView> = {}): TransactionView => ({
  id: "t1",
  account_id: "a1",
  date: "2026-11-02",
  amount_minor: -5000,
  description: "To savings",
  category_id: null,
  note: null,
  source: "import",
  import_id: null,
  fingerprint: null,
  balance_after_minor: null,
  category_source: null,
  account_name: "Current",
  category_name: null,
  category_kind: null,
  ...over,
});

const transfer = (over: Partial<TransferView> = {}): TransferView => ({
  id: "x1",
  out_txn_id: "t1",
  in_txn_id: null,
  status: "confirmed",
  origin: "rule",
  out_txn: txn(),
  in_txn: null,
  ...over,
});

describe("badgeText: the 'to review' count", () => {
  it("shows nothing when there is nothing to review", () => {
    expect(badgeText(0)).toBeNull();
  });
  it("shows the count, capped at 99+", () => {
    expect(badgeText(3)).toBe("3");
    expect(badgeText(99)).toBe("99");
    expect(badgeText(120)).toBe("99+");
  });
});

describe("sideText: one side of a Transfer", () => {
  it("says the other side is not imported yet when it is missing", () => {
    expect(sideText(null)).toBe("Other side not imported yet");
  });
  it("names the Account, date and description of a side", () => {
    expect(sideText(txn())).toBe("Current · 2026-11-02 · To savings");
  });
});

describe("linkCandidates: Transactions that can be linked by hand", () => {
  it("leaves out Transactions already in a Transfer with both sides, and the chosen one's own Account and sign", () => {
    const chosen = txn({ id: "c", account_id: "a1", amount_minor: -5000 });
    const sameAccount = txn({ id: "s", account_id: "a1", amount_minor: 5000 });
    const sameSign = txn({ id: "n", account_id: "a2", amount_minor: -5000 });
    const paired = txn({ id: "p", account_id: "a2", amount_minor: 5000 });
    const alone = txn({ id: "o", account_id: "a2", amount_minor: 5000 });
    const free = txn({ id: "f", account_id: "a3", amount_minor: 4900 });
    const transfers = [
      transfer({ out_txn_id: "q", in_txn_id: "p", out_txn: txn({ id: "q" }), in_txn: paired }),
      transfer({ out_txn_id: null, in_txn_id: "o", out_txn: null, in_txn: alone }),
    ];
    const ids = linkCandidates(chosen, [chosen, sameAccount, sameSign, paired, alone, free], transfers).map((t) => t.id);
    expect(ids).toEqual(["o", "f"]);
  });
});

describe("matchRows: a match's two versions side by side", () => {
  const manual = txn({
    id: "m1",
    source: "manual",
    account_name: "Card",
    date: "2026-11-09",
    amount_minor: -249,
    description: "Coffee with Ana",
    category_id: "c1",
    category_name: "Bars",
    note: "my treat",
  });
  const imported = txn({ id: "i1", account_name: "Card", date: "2026-11-10", amount_minor: -250, description: "BAR CENTRALE" });

  it("keeps the imported date, amount and description and the user's Category and note", () => {
    expect(matchRows(manual, imported, String)).toEqual([
      { label: "Date", manual: "2026-11-09", imported: "2026-11-10", merged: "2026-11-10" },
      { label: "Amount", manual: "-249", imported: "-250", merged: "-250" },
      { label: "Description", manual: "Coffee with Ana", imported: "BAR CENTRALE", merged: "BAR CENTRALE" },
      { label: "Category", manual: "Bars", imported: "Uncategorised", merged: "Bars" },
      { label: "Note", manual: "my treat", imported: "", merged: "my treat" },
    ]);
  });
  it("keeps the imported Category and note when the Manual Transaction has none", () => {
    const rows = matchRows(
      txn({ ...manual, category_id: null, category_name: null, note: null }),
      txn({ ...imported, category_id: "c2", category_name: "Groceries", note: "bank" }),
      String,
    );
    expect(rows.slice(3)).toEqual([
      { label: "Category", manual: "Uncategorised", imported: "Groceries", merged: "Groceries" },
      { label: "Note", manual: "", imported: "bank", merged: "bank" },
    ]);
  });
});
