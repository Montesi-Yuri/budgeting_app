# 16: Automatic Sync and discovery

**What to build:** Sync happens without the user thinking about it. When the phone app comes to the foreground, it syncs automatically if the PC can be reached. The PC announces itself on the local network, so the phone finds it even after its address has changed. On first use, the PC sets up a Windows Firewall rule for private networks only.

**Blocked by:** 15

**Status:** ready-for-agent

- [ ] When the phone app comes to the foreground, it tries a Sync with a short timeout (about 1.5 s) and fails quietly if the PC can't be reached
- [ ] While Sync is on, the PC announces itself on the local network. If the last known address fails, the phone looks for the paired PC by its device id before suggesting a QR rescan.
- [ ] A newly found address is saved as the PC's last known address
- [ ] Turning Sync on for the first time on the PC adds a Windows Firewall rule that allows incoming connections on private networks only, explaining why first
- [ ] The phone shows when the last successful Sync happened
- [ ] Manual verification: change the PC's IP address (for example by reconnecting to the network) and check that the phone still syncs without a rescan
