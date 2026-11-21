import { describe, expect, it } from "vitest";
import { backupFileName, restoreSummary } from "./backup";

describe("backupFileName names a Backup after the day it is made", () => {
  it("uses the local date", () => {
    expect(backupFileName(new Date(2026, 10, 3, 23, 30))).toBe("budget-2026-11-03.bsync.age");
  });
});

describe("restoreSummary reports what a restore did", () => {
  it("counts changes added and already present", () => {
    expect(restoreSummary({ added: 12, already_present: 3 })).toBe(
      "Backup restored: 12 changes added, 3 already present.",
    );
  });

  it("says when nothing was missing", () => {
    expect(restoreSummary({ added: 0, already_present: 40 })).toBe(
      "Nothing to restore: all 40 changes in the Backup were already present.",
    );
  });

  it("uses the singular for one change", () => {
    expect(restoreSummary({ added: 1, already_present: 1 })).toBe(
      "Backup restored: 1 change added, 1 already present.",
    );
  });
});
