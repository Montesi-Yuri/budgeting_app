# The Plan
1. I need to monitor expenses and income.
2. I can't sync with home banking so I should be able to upload a pdf/csv with expenses / incomes to easily get them in the system
3. I want to forecast how much I can spend for each month / week of the year
4. I want to have the app both on pc and mobile without having cloud infra. So I need to sync the data by connecting pc <-> phone and each device gets whats missing

# Development

## Prerequisites (Windows)

- Node.js 22+ and npm
- Rust (stable, MSVC toolchain) with the Visual Studio C++ build tools
- Microsoft Edge WebView2 runtime (preinstalled on Windows 11)

Then run `npm install`.

## Run

```sh
npm run tauri dev      # starts Vite and the app with hot reload
npm run tauri build    # release build and installer, in target/release/
```

The database is created on first launch at `%APPDATA%\app.budgeting.personal\budget.sqlite3`.

## Test and check

```sh
npm run test:rust      # Rust tests: HLC, write path and sync log, Accounts, settings
npm test               # UI unit tests (money formatting and parsing)
npm run typecheck      # TypeScript
cargo clippy --workspace --all-targets
```

## Layout

| Path | What it is |
|---|---|
| `core/` | Crate `budget_core`: the database, migrations, the single write path and sync log (`store.rs`), the hybrid logical clock (`hlc.rs`) and the domain modules (`accounts.rs`, `settings.rs`, `imports/` for CSV Imports, ...). It doesn't depend on Tauri. Tests are in `core/tests/`. |
| `core/migrations/` | SQL migrations, applied in order. Never edit a released one: add a new file. |
| `src-tauri/` | The Tauri shell: app setup and `#[tauri::command]`s (`commands.rs`). |
| `src/` | The React UI. `api.ts` wraps the commands, and `money.ts` formats and parses amounts in minor units. |

See [CONTEXT.md](CONTEXT.md) for the glossary, [docs/sync-design.md](docs/sync-design.md) for the data model and sync protocol, and [docs/adr/](docs/adr/) for decisions.
