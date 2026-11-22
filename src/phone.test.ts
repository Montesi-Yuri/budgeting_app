import { describe, expect, it } from "vitest";
import { defaultAccountId, isUnreachable, quickAmount, scannedPairingCode, syncTarget } from "./phone";
import type { AccountWithBalance, Peer } from "./api";

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

describe("scannedPairingCode: what the camera read", () => {
  it("accepts a pairing code, trimmed", () => {
    expect(scannedPairingCode(" bsync:1:abc\n")).toEqual({ ok: true, code: "bsync:1:abc" });
  });
  it("refuses any other QR code with a clear message", () => {
    expect(scannedPairingCode("https://example.com")).toEqual({
      ok: false,
      error: "This QR code is not a pairing code. On the PC, open Settings → Sync → Pair new device and scan the code shown there.",
    });
  });
});

describe("isUnreachable: whether a failed Sync should offer to rescan the QR code", () => {
  it("is true for the unreachable kind only", () => {
    expect(isUnreachable({ kind: "unreachable", message: "x" })).toBe(true);
    expect(isUnreachable({ kind: "sync", message: "x" })).toBe(false);
    expect(isUnreachable("boom")).toBe(false);
  });
});

const peer = (device_id: string, last_addr: string | null, last_sync_at: number | null): Peer => ({
  device_id,
  name: device_id,
  last_addr,
  paired_at: 1,
  last_sync_at,
});

describe("syncTarget: the Device the phone's Sync now button talks to", () => {
  it("is the paired Device with a known address, the most recently synced first", () => {
    expect(syncTarget([peer("old-pc", "10.0.0.2:47321", 100), peer("pc", "10.0.0.3:47321", 200)])?.device_id).toBe("pc");
  });
  it("skips Devices that connect to this one", () => {
    expect(syncTarget([peer("laptop", null, 300), peer("pc", "10.0.0.3:47321", null)])?.device_id).toBe("pc");
  });
  it("is null when there is nothing to connect to", () => {
    expect(syncTarget([peer("laptop", null, 300)])).toBeNull();
    expect(syncTarget([])).toBeNull();
  });
});
