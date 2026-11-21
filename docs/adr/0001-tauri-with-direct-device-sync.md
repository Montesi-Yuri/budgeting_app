# Tauri app with direct device-to-device sync, no server

The app must run on a Windows PC and an Android phone without any cloud infrastructure. We chose Tauri, so one Rust core holds all data and sync logic on both platforms. Each Device keeps a full SQLite copy, and the two Devices sync directly over the home Wi-Fi: the PC listens, the phone connects, Pairing happens once by QR code, and the traffic is encrypted with the Noise protocol. Changes travel as an append-only log of operations, and when the same row is edited on both Devices, the edit with the later hybrid logical clock value wins for the whole row.

## Considered options

- **A hosted backend or a cloud database:** rejected. The user doesn't want to run cloud infrastructure, and the data is personal financial data.
- **Syncthing or a shared cloud folder:** rejected. It needs extra software and setup by the user, and merge logic would still be needed.
- **A PWA with WebRTC:** rejected. A browser can't run a listener, and connecting the two devices (signaling) is awkward without a server.

## Consequences

- Sync only works when both Devices are on the same network. Syncing over the internet is out of scope.
- Edits are merged per row, not per field, so if the same row is edited on both Devices while offline, one of those edits is lost. This is acceptable because there is a single user.
- See [sync-design.md](../sync-design.md) for the details.
