# 12: Backup

**What to build:** The user can export a Backup at any time: an encrypted, password-protected file containing all the data, made from the sync log. They can restore a Backup into an empty or an existing database. Restoring only adds what's missing, because applying a change twice has no effect. On the PC, a Backup is also written automatically every week to a folder the user chooses.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] "Export backup" asks for a passphrase and a location, and writes one encrypted file
- [ ] "Restore backup" asks for the file and the passphrase, and applies every change in it. The result is reported as the number of changes added and already present.
- [ ] Restoring into the same database twice changes nothing the second time
- [ ] Restoring into an empty database on a new installation gives the same data as the original
- [ ] The Backup does not contain the Device's private key. After a restore on a new installation, its peers must be paired again.
- [ ] On the PC, the user can choose a folder for a weekly automatic Backup. The last 8 are kept. The passphrase is stored in the Windows credential store, so no prompt is needed. The folder setting stays on this Device and isn't synced.
- [ ] A wrong passphrase gives a clear error and changes nothing
