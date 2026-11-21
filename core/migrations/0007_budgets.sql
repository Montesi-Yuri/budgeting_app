-- 0007: Budgets. Schema per docs/sync-design.md §1.3.
--
-- category_id is not a foreign key constraint (see 0002): remote ops apply in
-- any order, and a Budget whose Category is deleted stays and is shown as
-- broken (sync-design §6).

CREATE TABLE budget (
  id            TEXT PRIMARY KEY,
  category_id   TEXT NOT NULL,              -- an expense Category; a parent covers its children
  per           TEXT NOT NULL CHECK (per IN ('period', 'week')),
  amount_minor  INTEGER NOT NULL,           -- positive limit
  valid_from    TEXT NOT NULL,              -- 'YYYY-MM-DD', first day it applies
  valid_to      TEXT,                       -- last day it applies; NULL = no end
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);
