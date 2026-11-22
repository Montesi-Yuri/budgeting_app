# Blocking TCP with threads for the Sync transport, in the core crate

Pairing and the Noise-encrypted Sync transport live in `budget_core::sync` (`SyncNode`, `Listener`), not in the Tauri shell, so the phone build (ticket 15) and automatic Sync (ticket 16) reuse them and the tests can pair two Stores over real localhost TCP in one process. They use blocking `std::net` sockets with one thread per connection and a writer thread per connection, instead of `tokio` as sync-design.md first suggested. A Device talks to one or two peers, a Sync takes well under a second, and the Sync session (`Session`) is already I/O-free, so an async runtime would add a large dependency and no benefit. The Tauri commands run the blocking calls on `spawn_blocking`.

The Store is shared as `Arc<Mutex<Store>>` and locked once per received message, not for a whole Sync, so the UI stays usable while a Sync runs.

## Considered options

- **`tokio` in the core crate:** rejected for now. If mDNS (ticket 16) needs a runtime, `Session` can still be driven from async code with split read/write halves.
- **The transport in the Tauri shell:** rejected. It could not be tested without Tauri, and the phone would need its own copy.

## Consequences

- Each open connection holds a thread. The listener accepts any number of connections, but only paired Devices (or one with the open pairing token) get past the handshake.
- The Noise private key is a file next to the database for now (see sync-design.md §2.4).
- mDNS discovery (ticket 16) uses `mdns-sd`, which runs its own thread with no async runtime, so this decision still holds.
