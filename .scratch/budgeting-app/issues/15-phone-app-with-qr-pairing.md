# 15: Phone app with QR pairing

**What to build:** The app runs on Android. On the PC, "Pair new device" shows the pairing payload as a QR code, and the phone scans it with its camera to pair. The phone's main uses are entering Manual Transactions on the spot, and checking Spendable and the Weekly Spendable. The phone connects to the PC and syncs in both directions.

**Blocked by:** 02, 14

**Status:** ready-for-agent

- [ ] The app builds and installs on Android, with the same data and sync code as the PC
- [ ] The PC's pairing screen shows the payload as a QR code, alongside the expiry time
- [ ] The phone has a "Pair with PC" option that opens the camera scanner, reads the QR code and completes Pairing as in ticket 14
- [ ] The camera and network permissions are requested on Android
- [ ] The phone's main screen is laid out for phone use: Spendable, Weekly Spendable and a quick "add Manual Transaction" form
- [ ] "Sync now" on the phone connects to the PC's last known address and syncs. If the PC can't be reached, a clear message suggests rescanning the QR code, which only refreshes the address and keeps the Pairing.
- [ ] Manual verification: a Manual Transaction entered on the phone appears on the PC after Sync, and a CSV imported on the PC appears on the phone
