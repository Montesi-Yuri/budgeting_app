-- 0010: Categorisation Rules. Schema per docs/sync-design.md §1.3.
--
-- account_id and category_id are not foreign key constraints (see 0002):
-- remote ops apply in any order, and a Rule whose Category is deleted stays
-- and is shown as broken (sync-design §6).

CREATE TABLE rule (
  id            TEXT PRIMARY KEY,
  position      REAL NOT NULL,              -- first match wins; REAL so a move writes one row
  match_text    TEXT NOT NULL,              -- case-insensitive "contains" on the description
  account_id    TEXT,                       -- optional filter
  direction     TEXT CHECK (direction IN ('in', 'out')), -- NULL = either
  action        TEXT NOT NULL CHECK (action IN ('categorise')), -- 'transfer' comes with ticket 06
  category_id   TEXT,                       -- for 'categorise'
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);
