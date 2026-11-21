# 13: Sync core

**What to build:** The heart of Sync, without networking: two databases exchange the changes each one is missing, using the protocol in the sync design. Each side sends a Hello with what it already has (the highest change number per Device), then the changes the other side is missing in batches, then Done. Applying a change is idempotent and order-independent. If the same row is edited on both sides, the edit with the later clock value wins. This ticket is verified entirely by tests.

**Blocked by:** 01

**Status:** resolved

- [x] Applying a change received from another Device records it in the sync log if it is new, and updates the row only if the change is newer than the row's current version
- [x] A sync session works over an in-memory channel: Hello, batches of changes in both directions, Done
- [x] Each received batch is applied in one DB transaction
- [x] A protocol version or schema mismatch produces a clear error and no changes
- [x] Property-style test: two databases with random independent edits, synced, end up with identical rows
- [x] Test: the same set of changes applied in different orders gives the same result
- [x] Test: a session cut off halfway, followed by a new session, finishes correctly without duplicating anything
- [x] Test: with three databases (A↔B, then B↔C), C receives A's changes

## Comments

**Implemented (2026-11-21).**

- **Apply path:** `Store::apply_remote(&[Op]) -> ApplyReport` in `core/src/store.rs`, with the logic in `core/src/sync/apply.rs`. It applies one batch in one DB transaction, and the clock observes every op. A row's state is a function of the set of ops for it in the log: the newest op decides `updated_hlc` and `deleted`, and the newest upsert decides the columns. A delete that arrives before the row it deletes (possible with three Devices) therefore still wins. One refinement of "update the row only if the change is newer": an older upsert can still fill in the columns of a tombstone that has no data yet. sync-design.md §1.4 now explains this.
- **Registry:** `SYNCED_ENTITIES` in `core/src/sync/registry.rs` maps `op.entity` back to its `SyncedRow`. To add an entity, add one line: `entity::<MyRow>(),`. The test `every_synced_table_is_registered_for_sync` fails for any table with `updated_hlc` that isn't listed. The steps are in the docs of `core/src/store.rs`.
- **Protocol:** `core/src/sync/protocol.rs` has `Msg` (Hello, Ops, Done, Error), `ErrCode`, `SyncError`, `PROTOCOL_VERSION = 1`, and `encode`/`decode` (CBOR). `Op` and `Hlc` are now serde types, and the payload is sent as CBOR bytes. A batch holds at most 200 ops and is capped at 60 KiB encoded, so it fits in one Noise frame.
- **Session:** `core/src/sync/session.rs`. `Session` is the protocol logic with no I/O: `Session::start(&store)` returns the Hello, `handle(&mut store, msg)` returns the replies, then `is_finished` and `report`. `sync::run(&mut store, &mut impl Transport)` drives it over a blocking transport. `sync::memory::pair()` is the in-memory channel. `SyncReport` has `peer_device_id`, `sent`, `received`, `entities` and `converged`. On a failure, `Msg::for_error` gives the Error message to send to the peer.
- **Mismatch:** a different `proto` gives `SyncError::IncompatibleProtocol`. A peer with a newer schema gives `SyncError::UpdateApp`, and the peer receives `ErrCode::UpdateApp` and shows "update the app on the other Device". A peer with an older schema is accepted.
- **Tests:** 21 new Rust tests in `core/tests/apply_remote.rs`, `sync_session.rs` and `sync_properties.rs`. The property tests use 40 seeds with a seeded RNG. 49 Rust tests and 23 UI tests in total. Clippy reports 0 warnings.
- **For ticket 14:** `Transport::send` must not block waiting for the peer to read, because each side sends all its batches before reading. Use a writer task or thread for TCP, or drive `Session` from async code with separate read and write halves. Pairing messages (PairRequest, PairOk) are not in `Msg` yet.
