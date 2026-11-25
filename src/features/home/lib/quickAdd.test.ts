import { describe, expect, it } from "vitest";
import type { AccountWithBalance } from "@/shared/api";
import { defaultAccountId, quickAmount } from "./quickAdd";

describe("quickAmount: the phone's quick add takes the amount without a sign", () => {
  it("money out is stored negative", () => {
    expect(quickAmount("12.50", "out", "EUR")).toEqual({ ok: true, minor: -1250 });
  });
  it("money in is stored positive", () => {
    expect(quickAmount("12,5", "in", "EUR")).toEqual({ ok: true, minor: 1250 });
  });
  it("a typed sign is ignored in favour of the chosen direction", () => {
    expect(quickAmount("-3", "in", "EUR")).toEqual({ ok: true, minor: 300 });
    expect(quickAmount("+3", "out", "EUR")).toEqual({ ok: true, minor: -300 });
  });
  it("zero is refused", () => {
    expect(quickAmount("0", "out", "EUR").ok).toBe(false);
  });
  it("text that isn't an amount is refused", () => {
    expect(quickAmount("abc", "out", "EUR").ok).toBe(false);
  });
});

const account = (id: string, kind: AccountWithBalance["kind"], is_main = false): AccountWithBalance => ({
  id,
  name: id,
  kind,
  opening_minor: 0,
  opening_date: "2026-01-01",
  is_main,
  balance_minor: 0,
});

describe("defaultAccountId: the Account preselected in quick add", () => {
  const accounts = [account("card", "credit_card"), account("main", "current", true), account("other", "current")];
  it("is the Account used last, while it still exists", () => {
    expect(defaultAccountId(accounts, "card")).toBe("card");
  });
  it("is the main Account when the last one was deleted or there is none", () => {
    expect(defaultAccountId(accounts, "gone")).toBe("main");
    expect(defaultAccountId(accounts, null)).toBe("main");
  });
  it("is the first Account when none is the main one", () => {
    expect(defaultAccountId([account("a", "savings"), account("b", "current")], null)).toBe("a");
  });
  it("is null with no Accounts", () => {
    expect(defaultAccountId([], "card")).toBeNull();
  });
});
