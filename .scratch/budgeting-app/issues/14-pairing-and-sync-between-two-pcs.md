# 14: Pairing and Sync between two PC instances

**What to build:** Two instances of the desktop app pair, then Sync over the network, encrypted with the Noise protocol. One instance (the listener) starts Pairing and shows the pairing payload: address, port, public key and a one-time token that expires after 5 minutes. For now, the other instance pastes that payload as text instead of scanning a QR code. Later Syncs only accept Devices that are already paired. The user sees the list of paired Devices, can unpair one, and can trigger "Sync now" with progress.

**Blocked by:** 13

**Status:** resolved

- [x] The listener can turn Sync on and off in settings. It only listens while Sync is on.
- [x] "Pair new device" on the listener creates the payload with a one-time token that expires after 5 minutes
- [x] The other instance pastes the payload, tries each address in it, completes the encrypted handshake, and sends the token. Both sides then store each other as paired Devices.
- [x] An expired, reused or wrong token is rejected
- [x] After Pairing, the first Sync runs immediately. Later, "Sync now" runs the Sync core from ticket 13 over the encrypted connection, and shows progress plus a summary of changes sent and received.
- [x] A connection from a Device that isn't paired is closed without exchanging any data
- [x] Unpairing removes the peer, and its later connections are rejected
- [x] After a Sync that received changes, the UI refreshes the affected views
- [x] Test: two instances on one machine, using different data folders and ports, pair and converge

## Comments

**Implemented (2026-11-21).**

- **Core, no Tauri:** `budget_core::sync::SyncNode` (`core/src/sync/node.rs`) does Pairing and Sync over TCP with Noise `IK_25519_ChaChaPoly_BLAKE2s` (crate `snow`, `core/src/sync/secure.rs`). `listen(addr) -> Listener` (Sync is on while it lives, `stop()` turns it off), `start_pairing(addrs) -> PairingPayload`, `cancel_pairing()`, `join(text) -> Joined { peer, report }` (pairs, then runs the first Sync on the same connection), `sync_with(device_id)` / `sync_with_addrs(id, extra)` for "Sync now", `peers()`, `unpair(id)`, `set_connect_timeout`. `SyncEvents` reports `progress`, `paired`, `synced` and `failed`, including for Syncs the other Device started. The networking is blocking `std::net` with threads, not tokio (ADR 0003). The Store is shared as `Arc<Mutex<Store>>` and locked once per message.
- **Payload:** `PairingPayload` (`core/src/sync/pairing.rs`) has the §2.1 fields and the text form `bsync:1:<base64url(CBOR)>`, ready for a QR code. The token is 32 random bytes and is valid for 5 minutes. It is used up by a successful Pairing and replaced by a new "Pair new device".
- **Protocol:** `Msg::PairRequest` and `Msg::PairOk`, and `ErrCode::PairingExpired` and `ErrCode::PairingInvalid`. The listener sees the joining Device's key in the first handshake message. An unknown key with no pairing open is disconnected before the listener sends anything back.
- **Keys and settings:** `Identity` is the Noise key pair, kept in `sync.key` next to the database (not in SQLite, so not in a Backup). Its public key fills `device.static_pubkey` of this Device. Peers are `device` rows (`core/src/sync/peers.rs`). The per-Device Sync on/off and port are in `sync.json` in the data folder, so **no migration was needed**.
- **Tauri shell:** `src-tauri/src/sync_commands.rs` has `sync_status`, `sync_server_set(enabled, port)`, `pairing_start`, `pairing_cancel`, `pairing_join(payload)`, `peers_list`, `peer_remove(device_id)` and `sync_now(device_id)`, and emits `sync://progress`, `sync://paired`, `sync://finished`, `sync://failed` and `data://changed {entities}`. `BUDGET_DATA_DIR` starts an instance with another data folder. `AppStore` is now `Arc<Mutex<Store>>`.
- **UI:** a Sync card in Settings (`src/components/SyncSettings.tsx`) with Allow Sync and the port, Pair new device (the code, a countdown, Copy and Cancel), Join another Device (paste), and the paired Devices with Sync now and Unpair, plus progress and a summary of what was sent and received. On `data://changed`, `App.tsx` remounts the views showing the changed entities (`viewsToRefresh` in `src/sync.ts`) and reloads the Currency.
- **Tests:** 19 new Rust tests. `core/tests/pairing_payload.rs` has 3. `core/tests/pairing_sync.rs` has 16, with two Stores pairing and syncing over real TCP on 127.0.0.1: first Sync, an expired, reused, wrong or replaced token, Sync now, 1,500 ops each way without blocking, unpairing on either side, pairing again with a new code, a raw Noise client with an unknown key getting 0 bytes back, Sync off, the listener's events, and two on-disk instances with their own data folders, key files and ports that keep their Pairing after a restart. There are also 15 new UI tests in `src/sync.test.ts`. In total that is 93 Rust tests and 50 UI tests. Clippy reports 0 warnings, and fmt passes.
- **Not verified by hand:** the UI was not tried in a running app, and no Sync was run between two processes or two physical PCs. To try two instances on one PC, see the README. Windows shows its firewall prompt the first time the app listens on `0.0.0.0`. Allow private networks there. The app doesn't add a firewall rule itself (ticket 16).
- **Open:** the private key should move to the Windows credential store and the Android Keystore. The listener stores no address for a Device that connects to it, so "Sync now" works only from the joining side. A fresh install must choose a Currency before it can reach Settings to pair, and its choice may override the other Device's Currency (the later write wins). Offering "Pair with another Device" on the welcome screen would fix this.
- **For ticket 15:** use `PairingPayload::to_text()` as the QR content and `SyncNode::join(scanned_text)` on the phone. A rescan with a fresh token refreshes the PC's row and `last_addr`, and the Pairing is kept. **For ticket 16:** use `set_connect_timeout(1.5 s)` and `sync_with_addrs(id, &[addr found by mDNS])`, which saves the address that worked. `sync_server_set` is the place to add the firewall rule.
