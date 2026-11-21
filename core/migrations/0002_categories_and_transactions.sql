-- 0002: Categories and Transactions. Schema per docs/sync-design.md §1.3.
--
-- References between synced rows (txn -> account, txn -> category,
-- category -> parent) are NOT foreign key constraints: remote ops must apply
-- in any order (sync-design §1.4), and a referenced row may be a tombstone.
-- The domain code checks references when the user writes, and reads treat a
-- reference to a deleted Category as Uncategorised.

CREATE TABLE category (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL CHECK (kind IN ('income', 'expense')), -- decides Income vs Expense
  parent_id     TEXT,                       -- max 2 levels: a parent can't have a parent
  color         TEXT,
  position      REAL NOT NULL,              -- display order among siblings
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE txn (
  id              TEXT PRIMARY KEY,         -- v7 if manual, v5 if imported (§1.5)
  account_id      TEXT NOT NULL,
  date            TEXT NOT NULL,            -- 'YYYY-MM-DD'; card purchases: purchase date
  amount_minor    INTEGER NOT NULL,         -- signed, as seen from the Account
  description     TEXT NOT NULL,
  note            TEXT,
  source          TEXT NOT NULL CHECK (source IN ('import', 'manual')),
  import_id       TEXT,                     -- the Import that created it (ticket 03)
  fingerprint     TEXT,                     -- imported rows only (§1.5)
  balance_after_minor INTEGER,              -- from the CSV, if present
  category_id     TEXT,                     -- NULL = Uncategorised
  category_source TEXT CHECK (category_source IN ('user', 'rule')), -- Rules never overwrite 'user'
  updated_hlc     TEXT NOT NULL,
  deleted         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX txn_account_date ON txn(account_id, date) WHERE deleted = 0;
CREATE INDEX txn_category ON txn(category_id) WHERE deleted = 0;
