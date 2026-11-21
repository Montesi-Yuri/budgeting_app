# 03: CSV Import with Import profiles

**What to build:** The user imports a bank CSV into an Account. The first time, they define an Import profile in the app: the columns for the date, amount and description, the date format, separator, decimal style, encoding and the rows to skip, with a live preview of the parsed rows. Later Imports for that Account reuse the profile. Imported Transactions get deterministic ids, so re-importing the same or overlapping files, even on another Device, never creates duplicates.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Choosing a CSV for an Account with no Import profile opens a setup step with a preview of the first rows as they would be parsed
- [ ] The profile supports: one signed amount column, or separate debit and credit columns; decimal comma or point; an optional thousands separator; several description columns joined together; an optional balance column; UTF-8 and Windows-1252 encodings
- [ ] The Import profile is saved per Account and synced like any other data. The user can edit it later.
- [ ] Each imported Transaction's id is derived from the Account, date, amount, normalised description and an occurrence index for identical lines on the same day. Rows whose id already exists are skipped.
- [ ] After an Import, a summary shows how many rows were new and how many were skipped, and the Import appears in an import history for that Account
- [ ] If the profile has a balance column and it disagrees with the calculated balance, the app warns about the first mismatched row
- [ ] The original file is not stored or synced
- [ ] Tests with sample CSVs covering: decimal comma, split debit/credit columns, a file imported twice, two files with overlapping date ranges, and two identical lines on the same day
