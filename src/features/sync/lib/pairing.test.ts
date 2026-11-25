import { describe, expect, it } from "vitest";
import type { Peer } from "@/shared/api";
import { isUnreachable, scannedPairingCode, syncTarget } from "./pairing";

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
