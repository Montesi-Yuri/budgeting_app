# A Tauri-free core crate that owns the single write path

All data, the sync log, the hybrid logical clock and the domain rules live in a plain Rust library, `core/` (crate `budget_core`), with no Tauri or UI dependency. The Tauri app in `src-tauri/` is a thin shell: it opens the database in the app data folder and exposes `#[tauri::command]` functions that lock the `Store` and call `budget_core`. Both crates sit in one Cargo workspace at the repo root.

Every change to synced data goes through one function, `Store::write(|w| ...)`. Inside it, `w.upsert(&row)` and `w.delete::<Row>(id)` stamp the change with the HLC, write the row with that stamp in `updated_hlc` (a delete only sets `deleted = 1`), and append an op with the next `seq` and the full row as CBOR. Everything written inside one `write` closure commits in a single DB transaction. A synced table is described by implementing the `SyncedRow` trait for a struct holding the full row. Nothing else may `INSERT`, `UPDATE` or `DELETE` a synced table.

Migrations are plain SQL files in `core/migrations/`, run in order by a small runner that keeps the count in SQLite's `user_version`. That count is the `schema` number a Device sends in a sync Hello.

## Considered options

- **All Rust code in the Tauri crate (the template default):** rejected. Tests would have to compile and link Tauri, and the logic shared with Android and the sync tests (two databases in one process) would be tied to the app shell.
- **The `rusqlite_migration` crate:** rejected for now. Its latest release depends on an older `rusqlite`, and two `rusqlite` versions can't link SQLite together. The runner is about 20 lines.
- **A generic "save any table" function driven by column lists:** rejected. Each `SyncedRow` writes its own SQL, which is explicit, and the op payload is just the struct's serde encoding.

## Consequences

- `cargo test -p budget-core` runs every data and sync test without Tauri.
- Later tickets add an entity by adding a migration, a struct and a `SyncedRow` implementation (see the docs in `core/src/store.rs`). The apply path for remote ops (ticket 13) must map `op.entity` back to that struct.
- `device.static_pubkey` is nullable in migration 0001, unlike in sync-design.md §1.3, because the Noise key is only created with Pairing (ticket 14).
- The HLC counter is written with 4 digits. If more than 9,999 changes are stamped in one millisecond, the clock moves on to the next millisecond so string order still equals clock order.
