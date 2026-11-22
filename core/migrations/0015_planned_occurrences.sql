-- 0015: Planned item occurrences that are no longer simply "expected".
-- Schema per docs/sync-design.md §1.3.
--
-- Only decisions are stored: occurrences that are still expected are
-- calculated from the planned_item schedule. planned_item_id / txn_id are
-- not foreign key constraints (see 0002): remote ops apply in any order.

CREATE TABLE planned_occurrence (
  id              TEXT PRIMARY KEY,         -- UUIDv5(planned_item_id | due_date) (fulfilment::occurrence_id)
  planned_item_id TEXT NOT NULL,
  due_date        TEXT NOT NULL,            -- the date the schedule gives it
  -- 'expected' is stored only to remember a rejected fulfilment (txn_id).
  state           TEXT NOT NULL CHECK (state IN ('expected', 'fulfilled', 'skipped', 'later')),
  txn_id          TEXT,                     -- the fulfilling (or rejected) Transaction
  link_status     TEXT CHECK (link_status IN ('suggested', 'confirmed', 'rejected')),
  later_date      TEXT,                     -- 'arrives later': the new expected date
  updated_hlc     TEXT NOT NULL,
  deleted         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX planned_occurrence_txn ON planned_occurrence(txn_id) WHERE deleted = 0;
