-- 0014: Matching Manual Transactions on Import.
-- Schema per docs/sync-design.md §1.3 `manual_match`.
--
-- manual_txn_id / imported_txn_id are not foreign key constraints (see
-- 0002): remote ops apply in any order. The id is the UUIDv5 of the pair
-- (manual_matches::match_id), so both Devices create the same row.

CREATE TABLE manual_match (
  id              TEXT PRIMARY KEY,
  manual_txn_id   TEXT NOT NULL,
  imported_txn_id TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('suggested', 'confirmed', 'rejected')),
  updated_hlc     TEXT NOT NULL,
  deleted         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX manual_match_manual ON manual_match(manual_txn_id) WHERE deleted = 0;
CREATE INDEX manual_match_imported ON manual_match(imported_txn_id) WHERE deleted = 0;
