-- 0009: Planned items. Schema per docs/sync-design.md §1.3.
--
-- account_id and category_id are not foreign key constraints (see 0002):
-- remote ops apply in any order. Expected occurrences are calculated from
-- rrule, start_date and end_date and never stored; the planned_occurrence
-- table for fulfilled, skipped and moved occurrences comes with ticket 09.

CREATE TABLE planned_item (
  id            TEXT PRIMARY KEY,
  account_id    TEXT NOT NULL,
  category_id   TEXT NOT NULL,              -- its kind says Income or Expense
  description   TEXT NOT NULL,
  amount_minor  INTEGER NOT NULL,           -- signed, as it will appear on the Account
  rrule         TEXT NOT NULL,              -- 'FREQ=MONTHLY;BYMONTHDAY=27', 'FREQ=MONTHLY;INTERVAL=3;BYMONTHDAY=1',
                                            -- 'FREQ=YEARLY;BYMONTH=6;BYMONTHDAY=15'; DTSTART = start_date
  start_date    TEXT NOT NULL,              -- 'YYYY-MM-DD'
  end_date      TEXT,                       -- last day it can occur; NULL = no end
  set_aside     INTEGER NOT NULL,           -- 1 = spread into monthly Set-asides
  updated_hlc   TEXT NOT NULL,
  deleted       INTEGER NOT NULL DEFAULT 0
);
