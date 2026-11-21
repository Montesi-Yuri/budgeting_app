-- 0005: Import profiles and the Import history. Schema per docs/sync-design.md §1.3.
--
-- Both tables are synced. As in 0002, references to other synced rows are not
-- foreign key constraints, so remote ops can apply in any order.
-- The original CSV file is never stored: only the profile, the resulting
-- Transactions (table txn) and this summary of each Import.

CREATE TABLE import_profile (
  id              TEXT PRIMARY KEY,          -- UUIDv5 of the Account: one profile per Account,
                                             -- the same id on every Device
  account_id      TEXT NOT NULL,
  name            TEXT NOT NULL,             -- "Intesa current account CSV"
  encoding        TEXT NOT NULL CHECK (encoding IN ('utf-8', 'windows-1252')),
  delimiter       TEXT NOT NULL,             -- ';' | ',' | '\t' (one character)
  skip_rows       INTEGER NOT NULL DEFAULT 0, -- lines before the header
  date_column     TEXT NOT NULL,
  date_format     TEXT NOT NULL,             -- '%d/%m/%Y'
  amount_mode     TEXT NOT NULL CHECK (amount_mode IN ('signed', 'split')),
  amount_column   TEXT,                      -- for 'signed'
  debit_column    TEXT,                      -- for 'split'
  credit_column   TEXT,                      -- for 'split'
  decimal_sep     TEXT NOT NULL CHECK (decimal_sep IN (',', '.')),
  thousands_sep   TEXT,                      -- '.' | ',' | ' ' | "'" | NULL
  description_columns TEXT NOT NULL,         -- JSON list, joined with ' '
  balance_column  TEXT,                      -- optional
  updated_hlc     TEXT NOT NULL,
  deleted         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX import_profile_account ON import_profile(account_id) WHERE deleted = 0;

CREATE TABLE import_batch (
  id            TEXT PRIMARY KEY,            -- UUIDv7
  account_id    TEXT NOT NULL,
  profile_id    TEXT NOT NULL,
  source_name   TEXT NOT NULL,               -- "estratto_2026_10.csv"
  imported_at   INTEGER NOT NULL,            -- Unix ms
  first_date    TEXT,
  last_date     TEXT,
  rows_new      INTEGER NOT NULL,
  rows_skipped  INTEGER NOT NULL,            -- already present (duplicates)
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX import_batch_account ON import_batch(account_id, imported_at) WHERE deleted = 0;
