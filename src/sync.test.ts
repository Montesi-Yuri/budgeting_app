import { describe, expect, it } from "vitest";
import { expiresIn, needsFirewallExplanation, progressText, syncSummary, viewsToRefresh } from "./sync";
import type { SyncStatus } from "./api";

const report = (sent: number, received: number) => ({
  peer_device_id: "pc",
  sent,
  received,
  entities: received ? ["account"] : [],
  converged: true,
});

describe("syncSummary: what a finished Sync did", () => {
  it.each([
    [0, 0, "Already up to date."],
    [1, 0, "Sent 1 change, received none."],
    [0, 1, "Sent none, received 1 change."],
    [3, 12, "Sent 3 changes, received 12 changes."],
  ])("sent %i, received %i", (sent, received, text) => {
    expect(syncSummary(report(sent, received))).toBe(text);
  });
});

describe("progressText", () => {
  it("counts the changes so far", () => {
    expect(progressText({ peer_device_id: "pc", phase: "exchanging", sent: 2, received: 5 })).toBe(
      "Syncing… sending 2, received 5 so far",
    );
  });
});

describe("expiresIn: the pairing code's countdown", () => {
  const exp = 1_800_000_300;
  it.each([
    [1_800_000_000_000, "5:00"],
    [1_800_000_241_500, "0:59"],
    [1_800_000_299_999, "0:01"],
    [1_800_000_300_000, null],
    [1_800_000_400_000, null],
  ])("at %i ms", (nowMs, text) => {
    expect(expiresIn(exp, nowMs)).toBe(text);
  });
});

describe("viewsToRefresh: the views showing what a Sync changed", () => {
  it("refreshes every view that shows an Account after Accounts changed", () => {
    expect(viewsToRefresh(["account"])).toEqual(["forecast", "review", "accounts", "transactions", "uncategorised", "import", "planned", "rules"]);
  });
  it("refreshes Categories and the lists showing their names after Categories changed", () => {
    expect(viewsToRefresh(["category"])).toEqual(["forecast", "transactions", "uncategorised", "categories", "budgets", "planned", "rules"]);
  });
  it("refreshes the lists and the balances after Transactions changed", () => {
    expect(viewsToRefresh(["txn"])).toEqual(["forecast", "review", "accounts", "transactions", "uncategorised", "budgets"]);
  });
  it("refreshes everything for an entity it doesn't know yet", () => {
    expect(viewsToRefresh(["some_future_entity"])).toEqual(["forecast", "review", "accounts", "transactions", "uncategorised", "categories", "budgets", "import", "planned", "rules"]);
  });
  it("refreshes Budgets after Budgets changed and Import after Import profiles changed", () => {
    expect(viewsToRefresh(["budget"])).toEqual(["forecast", "budgets"]);
    expect(viewsToRefresh(["rule"])).toEqual(["rules"]);
    expect(viewsToRefresh(["import_profile", "import_batch"])).toEqual(["import"]);
  });
  it("refreshes the review list, the Uncategorised list and Budgets after Transfers changed", () => {
    expect(viewsToRefresh(["transfer"])).toEqual(["forecast", "review", "uncategorised", "budgets"]);
  });
  it("refreshes the review list after Planned item occurrences or Planned items changed", () => {
    expect(viewsToRefresh(["planned_occurrence"])).toEqual(["forecast", "review"]);
    expect(viewsToRefresh(["planned_item"])).toEqual(["forecast", "review", "planned"]);
  });
  it("refreshes nothing when nothing changed", () => {
    expect(viewsToRefresh([])).toEqual([]);
  });
});

describe("firewallExplanation: the first time Sync is turned on, explain the Windows Firewall rule first", () => {
  const status = (over: Partial<SyncStatus>): SyncStatus => ({
    enabled: false,
    port: 47321,
    listening: false,
    addrs: ["192.168.1.20"],
    announced: false,
    firewall: "missing",
    firewall_asked: false,
    ...over,
  });
  it("turning Sync on without the rule explains it first", () => {
    expect(needsFirewallExplanation(status({}), true)).toBe(true);
  });
  it("not when the rule is there already", () => {
    expect(needsFirewallExplanation(status({ firewall: "allowed" }), true)).toBe(false);
  });
  it("not off Windows", () => {
    expect(needsFirewallExplanation(status({ firewall: "not_needed" }), true)).toBe(false);
  });
  it("only once: not after the user answered it", () => {
    expect(needsFirewallExplanation(status({ firewall_asked: true }), true)).toBe(false);
  });
  it("not when turning Sync off", () => {
    expect(needsFirewallExplanation(status({ enabled: true }), false)).toBe(false);
  });
});
