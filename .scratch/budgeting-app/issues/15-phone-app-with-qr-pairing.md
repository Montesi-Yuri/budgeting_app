# 15: Phone app with QR pairing

**What to build:** The app runs on Android. On the PC, "Pair new device" shows the pairing payload as a QR code, and the phone scans it with its camera to pair. The phone's main uses are entering Manual Transactions on the spot, and checking Spendable and the Weekly Spendable. The phone connects to the PC and syncs in both directions.

**Blocked by:** 02, 14

**Status:** ready-for-human

- [ ] The app builds and installs on Android, with the same data and sync code as the PC
- [x] The PC's pairing screen shows the payload as a QR code, alongside the expiry time
- [ ] The phone has a "Pair with PC" option that opens the camera scanner, reads the QR code and completes Pairing as in ticket 14
- [ ] The camera and network permissions are requested on Android
- [ ] The phone's main screen is laid out for phone use: Spendable, Weekly Spendable and a quick "add Manual Transaction" form
- [x] "Sync now" on the phone connects to the PC's last known address and syncs. If the PC can't be reached, a clear message suggests rescanning the QR code, which only refreshes the address and keeps the Pairing.
- [ ] Manual verification: a Manual Transaction entered on the phone appears on the PC after Sync, and a CSV imported on the PC appears on the phone

## Comments

**Implemented what can be built and tested without an Android toolchain (2026-11-22). Status is `ready-for-human`:** this machine has no Java, Android SDK/NDK or Android Rust targets, and they weren't installed (system setup is the user's call). The setup and build commands are in the README under "Phone (Android)".

- **QR code (PC):** `PairingPayload::to_qr() -> Qr` and `Qr::to_svg()` in `core/src/sync/pairing.rs` (crate `qrcode`, medium error correction, 4-module quiet zone). `pairing_start` now also returns `qr_svg`, and "Pair new device" shows the QR code with the expiry time ("expires at 14:35 (in 4:59)") above the pasteable code (`src/components/PairingQr.tsx`). It stays black on white in dark mode. `core/tests/pairing_qr.rs` decodes the QR code with an independent reader (`rqrr`, dev-dependency only) and gets back the exact payload text.
- **Unreachable PC and rescan:** "Sync now" that reaches no address now fails with `PairingError::PeerUnreachable { name, addrs }`. Its message suggests scanning the QR code again and says that this keeps the Pairing. The shell maps it to the error kind `unreachable`, and the UI (Settings → Sync and the phone's Home) then shows a "Rescan the PC's QR code (keeps the Pairing)" button. A new test in `core/tests/pairing_sync.rs` covers it over real TCP: the PC stops and listens again on a new port, Sync now fails with `PeerUnreachable`, the rescan keeps one Pairing on each side, refreshes `last_addr`, syncs the data, and the next Sync now works. The old test for Sync turned off now expects `PeerUnreachable`.
- **Phone build:** `tauri-plugin-barcode-scanner` is a dependency only for `cfg(any(target_os = "android", target_os = "ios"))` and is registered under `#[cfg(mobile)]`. `capabilities/mobile.json` grants `barcode-scanner:default` on android/iOS only, and the desktop build accepts it. `app_info` now also returns `platform` and `mobile`. When `mobile` is true, the UI uses the phone layout: fewer tabs (Home, Transactions, Accounts, Categories, Settings), touch-sized controls, and `PhoneHome` (Spendable and Weekly Spendable, the quick add form `QuickAdd`, and "Sync now with <PC>" with the last Sync time). Quick add takes the amount without a sign, with a Money out / Money in toggle (money out by default), and preselects the last used Account (browser storage, falling back to the main Account). "Pair with PC" (`PairWithPc.tsx`) asks for the camera permission (`requestPermissions`), scans with the full-screen scanner restricted to QR codes, checks that the content is a `bsync:` code, and calls `pairing_join`. Pasting the code is still possible. `bundle.android.minSdkVersion` is 24. On Android the device's own name defaults to "Phone", because Android sets neither `COMPUTERNAME` nor `HOSTNAME`.
- **Welcome screen:** a new phone can "Pair with PC" before choosing a Currency, so it gets the PC's Currency instead of overriding it (the gap from ticket 14). The desktop welcome screen also offers "Or pair with another Device" (paste).
- **Permissions:** the camera permission is declared by the scanner plugin's Android manifest (merged at build time) and asked at the first scan. `INTERNET` is in Tauri's generated manifest. Neither could be checked because `src-tauri/gen/android` can't be generated here.
- **Tests:** 3 new Rust tests (2 in `pairing_qr.rs`, 1 in `pairing_sync.rs`) and 15 new UI tests in `src/phone.test.ts` (quick add amounts, the default Account, checking scanned content, the unreachable kind, choosing the Device to sync with). In total: 169 Rust tests and 93 UI tests. Clippy reports 0 warnings, and fmt, typecheck and the build pass.
- **Not verified:** no Android build, install or scan was run, and the phone layout has not been seen on a device (it renders only when `mobile` is true). The UI was not tried by hand on the PC either.

**Remaining (needs the Android toolchain and a phone):**
1. Install the toolchain and run `npx tauri android init` (README, "Phone (Android)"), then commit `src-tauri/gen/android`.
2. `npx tauri android dev` on the phone: check that it builds and installs, that the first scan asks for the camera, and that "Pair with PC" scans the PC's QR code and pairs (criteria 1, 3, 4).
3. Check the phone Home layout on the device (criterion 5).
4. Manual verification (criterion 7): a Manual Transaction added on the phone appears on the PC after Sync now, and a CSV imported on the PC appears on the phone. Stop Sync on the PC to check the rescan message.
5. Still open from ticket 14: the Noise key is a file (`sync.key`, in the app's private folder on Android), not in the Android Keystore. The weekly Backup reports itself unavailable off Windows.

**For ticket 16:** the phone starts every Sync: use `syncTarget(peers)` (`src/phone.ts`) and `sync_now`. Treat the error kind `unreachable` as "try discovery, then suggest rescanning". `PhoneHome` already reloads on `visibilitychange`, so the foreground Sync can be hooked there.
