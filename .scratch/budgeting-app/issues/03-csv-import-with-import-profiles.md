# 03: CSV Import with Import profiles

**What to build:** The user imports a bank CSV into an Account. The first time, they define an Import profile in the app: the columns for the date, amount and description, the date format, separator, decimal style, encoding and the rows to skip, with a live preview of the parsed rows. Later Imports for that Account reuse the profile. Imported Transactions get deterministic ids, so re-importing the same or overlapping files, even on another Device, never creates duplicates.

**Blocked by:** 02

**Status:** resolved

- [x] Choosing a CSV for an Account with no Import profile opens a setup step with a preview of the first rows as they would be parsed
- [x] The profile supports: one signed amount column, or separate debit and credit columns; decimal comma or point; an optional thousands separator; several description columns joined together; an optional balance column; UTF-8 and Windows-1252 encodings
- [x] The Import profile is saved per Account and synced like any other data. The user can edit it later.
- [x] Each imported Transaction's id is derived from the Account, date, amount, normalised description and an occurrence index for identical lines on the same day. Rows whose id already exists are skipped.
- [x] After an Import, a summary shows how many rows were new and how many were skipped, and the Import appears in an import history for that Account
- [x] If the profile has a balance column and it disagrees with the calculated balance, the app warns about the first mismatched row
- [x] The original file is not stored or synced
- [x] Tests with sample CSVs covering: decimal comma, split debit/credit columns, a file imported twice, two files with overlapping date ranges, and two identical lines on the same day

## Comments

**Implemented (2026-11-21).**

- Migration `core/migrations/0005_imports.sql` (number reserved; 0003/0004 belong to tickets 13/12) adds the synced tables `import_profile` and `import_batch` (sync-design §1.3, with `updated_hlc` and `deleted`, no foreign keys).
- `budget_core::imports`:
  - `ImportProfile` (entity `import_profile`): one per Account, id = `profile_id(account_id)` (UUIDv5), so a profile set up on two Devices is one row. `ImportProfile::draft(account_id)` is the starting point; `save_profile` validates and creates or replaces it; `profile_for_account` reads it.
  - `preview(store, &profile, bytes)`: parses without saving; returns the first lines, the header's columns, a whole-file `problem` (e.g. a missing column), the first 20 rows with a per-row `error`, and counts.
  - `import_file(store, account_id, file_name, bytes)`: parses with the saved profile, derives `fingerprint = sha256(account|date|amount|normalised description|n)` and `id = UUIDv5(fingerprint)` (`budget_core::ids`), skips rows whose id already exists (deleted ones too, so a deleted imported Transaction doesn't come back), and records an `ImportBatch` (entity `import_batch`) in the same write. Returns `ImportSummary { batch, balance_mismatch }`. A file with any unreadable line imports nothing; lines without a date (footers) are ignored.
  - `history(store, account_id)`: the Account's Imports, newest first.
  - Balance check: with a balance column, the file's balance after the last line of each day is compared with the Opening balance plus the Account's Transactions up to that day; the first mismatch is reported. Files may be oldest or newest first.
- Parsing (`imports/parse.rs`): UTF-8 (BOM stripped) and Windows-1252; one-character separator; lines to skip before the header; columns by header name; strftime-style dates (`%d %m %Y %y`, time parts ignored); decimal comma or point, optional thousands separator (grouping checked), leading/trailing minus, brackets and currency symbols; signed amount or debit/credit columns (debits are money out with or without a minus); several description columns joined with a space.
- Description normalisation for the fingerprint: lower case, white space collapsed.
- The file is read in the UI, sent as bytes to the command and dropped; it is never stored or synced.
- UI: a new Import page (Account, file, setup step with a live preview, Edit Import profile, Import summary with the balance warning, Import history). `src/imports.ts` guesses the separator and the columns from common header names.
- Sync (ticket 13): register the entities `import_profile` (`imports::ImportProfile`) and `import_batch` (`imports::ImportBatch`).
- For ticket 04 (Rules) and 07 (matching): `import_file` builds the new Transactions in a list before the single `store.write`; apply Rules to the new ones there and find matches on the new ids after it.
- Tests: 71 Rust tests (18 new in `core/tests/imports.rs`) and 46 UI tests (11 new). Not checked by launching the app, to keep test data out of the real app data folder.
