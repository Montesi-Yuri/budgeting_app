import { describe, expect, it } from "vitest";
import { lastSyncText, shouldAutoSync } from "./autoSync";
import type { Peer } from "./api";

const pc: Peer = {
  device_id: "pc",
  name: "Yuri-PC",
  last_addr: "192.168.1.20:47321",
  paired_at: 1_800_000_000,
  last_sync_at: null,
};

const NOW = 1_800_000_000_000;

describe("shouldAutoSync: the phone syncs when it comes to the foreground", () => {
  it("syncs the first time the app is in the foreground", () => {
    expect(shouldAutoSync(pc, { syncing: false, lastAttemptMs: null }, NOW)).toBe(true);
  });
  it("needs a paired PC", () => {
    expect(shouldAutoSync(null, { syncing: false, lastAttemptMs: null }, NOW)).toBe(false);
  });
  it("doesn't start a second Sync while one runs", () => {
    expect(shouldAutoSync(pc, { syncing: true, lastAttemptMs: null }, NOW)).toBe(false);
  });
  it("doesn't try again when the app comes back within 15 seconds", () => {
    expect(shouldAutoSync(pc, { syncing: false, lastAttemptMs: NOW - 14_000 }, NOW)).toBe(false);
  });
  it("tries again once 15 seconds have passed", () => {
    expect(shouldAutoSync(pc, { syncing: false, lastAttemptMs: NOW - 15_000 }, NOW)).toBe(true);
  });
});

describe("lastSyncText: when the last successful Sync happened", () => {
  const nowSecs = NOW / 1000;
  it("never synced", () => {
    expect(lastSyncText(null, NOW)).toBe("never");
  });
  it("less than a minute ago", () => {
    expect(lastSyncText(nowSecs - 59, NOW)).toBe("just now");
  });
  it("minutes ago", () => {
    expect(lastSyncText(nowSecs - 60, NOW)).toBe("1 minute ago");
    expect(lastSyncText(nowSecs - 42 * 60, NOW)).toBe("42 minutes ago");
  });
  it("hours ago, within a day", () => {
    expect(lastSyncText(nowSecs - 60 * 60, NOW)).toBe("1 hour ago");
    expect(lastSyncText(nowSecs - 23 * 60 * 60 - 59 * 60, NOW)).toBe("23 hours ago");
  });
  it("older: the date and time", () => {
    const text = lastSyncText(nowSecs - 3 * 24 * 60 * 60, NOW);
    expect(text).not.toMatch(/ago|just now|never/);
    expect(text.length).toBeGreaterThan(0);
  });
});
