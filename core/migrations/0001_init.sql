-- 0001: Devices, synced settings, Accounts and the sync log.
-- Schema per docs/sync-design.md §1.3. Every synced table has
-- `updated_hlc` (stamp of the change that last wrote the row) and
-- `deleted` (1 = tombstone; rows are never physically deleted).

-- This Device and (later) its paired peers. Not synced.
CREATE TABLE device (
  device_id     TEXT PRIMARY KEY,           -- UUIDv4, created on first launch
  name          TEXT NOT NULL,
  static_pubkey BLOB,                       -- Noise X25519 public key; set up by Pairing (ticket 14)
  is_self       INTEGER NOT NULL DEFAULT 0,
  last_addr     TEXT,
  paired_at     INTEGER,
  last_sync_at  INTEGER
);
CREATE UNIQUE INDEX device_one_self ON device(is_self) WHERE is_self = 1;

-- Synced key/value settings, e.g. 'currency' = "EUR". Value is JSON.
CREATE TABLE setting (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE account (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL CHECK (kind IN ('current', 'savings', 'credit_card')),
  opening_minor INTEGER NOT NULL,           -- Opening balance in minor units (card debt is negative)
  opening_date  TEXT NOT NULL,              -- 'YYYY-MM-DD'
  is_main       INTEGER NOT NULL DEFAULT 0,
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);

-- The sync log: append-only, one op per change.
CREATE TABLE op (
  origin        TEXT NOT NULL,              -- device_id that made the change
  seq           INTEGER NOT NULL,           -- 1,2,3... per origin, no gaps
  hlc           TEXT NOT NULL,
  entity        TEXT NOT NULL,              -- 'account' | 'setting' | ...
  entity_id     TEXT NOT NULL,
  kind          TEXT NOT NULL CHECK (kind IN ('upsert', 'delete')),
  payload       BLOB,                       -- CBOR of the full row (upsert only)
  PRIMARY KEY (origin, seq)
);
CREATE INDEX op_entity ON op(entity, entity_id);
