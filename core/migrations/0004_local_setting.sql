-- 0004: Per-device settings, such as the folder for the weekly automatic
-- Backup. This table is NOT synced and not in a Backup: it has no
-- `updated_hlc`/`deleted`, and nothing writes it through the write path.
-- (Numbered 0004 because tickets 02 and 13 add 0002 and 0003.)
CREATE TABLE local_setting (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL              -- JSON
);
