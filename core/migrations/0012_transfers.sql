-- 0012: Transfers, and Rules that mark Transactions as part of a Transfer.
-- Schema per docs/sync-design.md §1.3.
--
-- out_txn_id / in_txn_id are not foreign key constraints (see 0002): remote
-- ops apply in any order. A Transfer may have only one side until the other
-- Account is imported.

CREATE TABLE transfer (
  id            TEXT PRIMARY KEY,           -- UUIDv5 of the pair, or of the one side (transfers::pair_id / side_id)
  out_txn_id    TEXT,                       -- NULL until that Account is imported
  in_txn_id     TEXT,                       -- NULL until that Account is imported
  status        TEXT NOT NULL CHECK (status IN ('suggested', 'confirmed', 'rejected')),
  origin        TEXT NOT NULL CHECK (origin IN ('rule', 'match', 'user')),
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX transfer_out ON transfer(out_txn_id) WHERE deleted = 0;
CREATE INDEX transfer_in ON transfer(in_txn_id) WHERE deleted = 0;

-- 0010 allowed only action = 'categorise'. SQLite can't alter a CHECK, so
-- the table is rebuilt with 'transfer' allowed (category_id is NULL for it).
CREATE TABLE rule_new (
  id            TEXT PRIMARY KEY,
  position      REAL NOT NULL,
  match_text    TEXT NOT NULL,
  account_id    TEXT,
  direction     TEXT CHECK (direction IN ('in', 'out')),
  action        TEXT NOT NULL CHECK (action IN ('categorise', 'transfer')),
  category_id   TEXT,                       -- for 'categorise'
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);
INSERT INTO rule_new (id, position, match_text, account_id, direction, action, category_id,
                      updated_hlc, deleted)
  SELECT id, position, match_text, account_id, direction, action, category_id,
         updated_hlc, deleted
  FROM rule;
DROP TABLE rule;
ALTER TABLE rule_new RENAME TO rule;
