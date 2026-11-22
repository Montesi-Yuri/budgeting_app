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

## Pairing and Sync

In Settings → Sync, turn on **Allow Sync** on the Device that listens (the PC), then use **Pair new device** and paste the pairing code into **Join another Device** on the other one, within 5 minutes. The first time the app listens, Windows asks whether to allow it on the network: allow it for private networks only. Nothing else in the system's network settings is changed (ticket 16 will add a firewall rule).

To try it with two instances on one PC, start the second with its own data folder and use a different port in its Settings:

```sh
set BUDGET_DATA_DIR=%TEMP%\budget-second && target\debug\budgeting-app.exe
```

The pairing code lists the PC's private network addresses (or `127.0.0.1` if it has none), and either works between two instances on one PC. A second instance started with `npm run tauri dev` would clash with the first one's dev server, so use the built app.

## Phone (Android)

The phone build is the same app: the same `core/` data and Sync code, the phone layout (Spendable, Weekly Spendable, quick add and Sync now on Home) and a camera scanner for the PC's pairing QR code (`tauri-plugin-barcode-scanner`, phone build only). On the PC, **Pair new device** shows the QR code; on the phone, **Pair with PC** scans it. If **Sync now** can't reach the PC, the phone offers to rescan the QR code, which only refreshes the PC's address and keeps the Pairing.

One-time setup (Windows, no admin needed for the SDK itself):

1. Install [Android Studio](https://developer.android.com/studio). In its SDK Manager install the Android SDK Platform (API 34 or newer), Android SDK Platform-Tools, Android SDK Command-line Tools and **NDK (Side by side)**.
2. Set the environment variables (PowerShell, then open a new terminal):
   ```powershell
   [Environment]::SetEnvironmentVariable("JAVA_HOME", "C:\Program Files\Android\Android Studio\jbr", "User")
   [Environment]::SetEnvironmentVariable("ANDROID_HOME", "$env:LocalAppData\Android\Sdk", "User")
   $ndk = Get-ChildItem "$env:LocalAppData\Android\Sdk\ndk" | Sort-Object Name | Select-Object -Last 1
   [Environment]::SetEnvironmentVariable("NDK_HOME", $ndk.FullName, "User")
   ```
3. Add the Rust targets: `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`
4. Generate the Android project once, and commit `src-tauri/gen/android`: `npx tauri android init`

Build and run:

```sh
npx tauri android dev      # on a phone connected with USB debugging (or an emulator)
npx tauri android build --apk --target aarch64   # release APK in src-tauri/gen/android/app/build/outputs/apk/
```

The camera permission comes from the scanner plugin's manifest and is asked for the first time you scan; the network permission (`INTERNET`) is in the generated manifest. The phone and the PC must be on the same Wi-Fi, with Allow Sync on the PC.

## Test and check

```sh
npm run test:rust      # Rust tests: HLC, write path and sync log, domain modules, Backup, Sync and Pairing over localhost TCP
npm test               # UI unit tests (money formatting and parsing, Backup messages, Sync helpers)
npm run typecheck      # TypeScript
cargo clippy --workspace --all-targets
```

## Layout

| Path | What it is |
|---|---|
| `core/` | Crate `budget_core`: the database, migrations, the single write path and sync log (`store.rs`), the hybrid logical clock (`hlc.rs`) and the domain modules (`accounts.rs`, `settings.rs`, `period.rs` and `spendable.rs` for Periods, Weeks and Spendable, `imports/` for CSV Imports, `backup.rs` for Backups, ...). It doesn't depend on Tauri. Tests are in `core/tests/`. |
| `core/migrations/` | SQL migrations, applied in order. Never edit a released one: add a new file. |
| `src-tauri/` | The Tauri shell: app setup and `#[tauri::command]`s (`commands.rs`; the Backup commands, weekly Backup thread and Windows credential store in `backup_commands.rs`). |
| `src/` | The React UI. `api.ts` wraps the commands, and `money.ts` formats and parses amounts in minor units. The phone build uses `PhoneHome.tsx`, `QuickAdd.tsx` and `PairWithPc.tsx` (camera scanner in `scanner.ts`, helpers in `phone.ts`). |

See [CONTEXT.md](CONTEXT.md) for the glossary, [docs/sync-design.md](docs/sync-design.md) for the data model and sync protocol, and [docs/adr/](docs/adr/) for decisions.
