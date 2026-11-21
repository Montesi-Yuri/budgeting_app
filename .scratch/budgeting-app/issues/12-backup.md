# 12: Backup

**What to build:** The user can export a Backup at any time: an encrypted, password-protected file containing all the data, made from the sync log. They can restore a Backup into an empty or an existing database. Restoring only adds what's missing, because applying a change twice has no effect. On the PC, a Backup is also written automatically every week to a folder the user chooses.

**Blocked by:** 01

**Status:** resolved

- [x] "Export backup" asks for a passphrase and a location, and writes one encrypted file
- [x] "Restore backup" asks for the file and the passphrase, and applies every change in it. The result is reported as the number of changes added and already present.
- [x] Restoring into the same database twice changes nothing the second time
- [x] Restoring into an empty database on a new installation gives the same data as the original
- [x] The Backup does not contain the Device's private key. After a restore on a new installation, its peers must be paired again.
- [x] On the PC, the user can choose a folder for a weekly automatic Backup. The last 8 are kept. The passphrase is stored in the Windows credential store, so no prompt is needed. The folder setting stays on this Device and isn't synced.
- [x] A wrong passphrase gives a clear error and changes nothing

## Comments

**Implemented (2026-11-21).**

- Core: `core/src/backup.rs` (`budget_core::backup`): `export`, `restore` -> `RestoreReport { added, already_present }`, the weekly Backup (`run_weekly`, `latest_weekly`, `schedule_folder` / `set_schedule_folder`) and `file_name`. A Backup is the whole op log as CBOR, encrypted with `age` (passphrase/scrypt). It has no `device` rows and no keys, so peers are not restored. This differs from the first draft of sync-design §4, which I updated.
- Restore applies each op like `apply_remote` in sync-design §1.4, in HLC order and in one DB transaction: skip it if `(origin, seq)` is already there, otherwise add it to the log and write the row only if the op is newer than the row's `updated_hlc`. It is generic over the schema. `op.entity` is the table name, the row comes from the CBOR field names (= columns), and the primary key comes from `pragma_table_info`. So the tables added by 02 and later tickets need no Backup code, and a test checks this with a table only the test knows about. A change to a table this database doesn't have is refused, and nothing is changed. The clock observes the newest restored HLC. **When merging with ticket 13:** restore should call 13's apply path if it has one, or 13 can reuse this one.
- Per-device setting: migration `0004_local_setting.sql` adds an unsynced `local_setting` table that holds `backup_dir`. On this branch it is the 2nd migration (user_version 2). After merging with 02/03 it becomes the 4th, so throw away dev databases made on this branch.
- Shell: `src-tauri/src/backup_commands.rs` adds the commands `backup_export`, `backup_restore`, `backup_schedule_get` and `backup_schedule_set`. The weekly passphrase goes in the Windows credential store (`keyring`, service `app.budgeting.personal`). A background thread runs the weekly Backup on app start and then every hour. On non-Windows builds the weekly Backup is reported as unavailable. File pickers come from `tauri-plugin-dialog`.
- UI: Settings has a Backup card (Export, Restore, Weekly Backup), and the first-launch screen offers "Or restore a Backup".
- Tests: 42 Rust tests (14 new, in `core/tests/backup.rs` and `core/tests/backup_schedule.rs`) and 27 UI tests (4 new, in `src/backup.test.ts`).
- Not verified by hand in the running app: the dialogs, the credential store and the background thread. I didn't run the app, to keep test data out of the real app-data folder.
- `age` brings in `proc-macro-error2`, so cargo prints a future-incompatibility notice. It isn't a clippy warning.
- "Once a week" counts from the date in the newest `budget-YYYY-MM-DD.bsync.age` in the folder, using the UTC date.
