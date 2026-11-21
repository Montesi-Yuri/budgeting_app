# 14: Pairing and Sync between two PC instances

**What to build:** Two instances of the desktop app pair, then Sync over the network, encrypted with the Noise protocol. One instance (the listener) starts Pairing and shows the pairing payload: address, port, public key and a one-time token that expires after 5 minutes. For now, the other instance pastes that payload as text instead of scanning a QR code. Later Syncs only accept Devices that are already paired. The user sees the list of paired Devices, can unpair one, and can trigger "Sync now" with progress.

**Blocked by:** 13

**Status:** ready-for-agent

- [ ] The listener can turn Sync on and off in settings. It only listens while Sync is on.
- [ ] "Pair new device" on the listener creates the payload with a one-time token that expires after 5 minutes
- [ ] The other instance pastes the payload, tries each address in it, completes the encrypted handshake, and sends the token. Both sides then store each other as paired Devices.
- [ ] An expired, reused or wrong token is rejected
- [ ] After Pairing, the first Sync runs immediately. Later, "Sync now" runs the Sync core from ticket 13 over the encrypted connection, and shows progress plus a summary of changes sent and received.
- [ ] A connection from a Device that isn't paired is closed without exchanging any data
- [ ] Unpairing removes the peer, and its later connections are rejected
- [ ] After a Sync that received changes, the UI refreshes the affected views
- [ ] Test: two instances on one machine, using different data folders and ports, pair and converge
