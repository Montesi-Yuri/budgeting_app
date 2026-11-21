# 13: Sync core

**What to build:** The heart of Sync, without networking: two databases exchange the changes each one is missing, using the protocol in the sync design. Each side sends a Hello with what it already has (the highest change number per Device), then the changes the other side is missing in batches, then Done. Applying a change is idempotent and order-independent. If the same row is edited on both sides, the edit with the later clock value wins. This ticket is verified entirely by tests.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Applying a change received from another Device records it in the sync log if it is new, and updates the row only if the change is newer than the row's current version
- [ ] A sync session works over an in-memory channel: Hello, batches of changes in both directions, Done
- [ ] Each received batch is applied in one DB transaction
- [ ] A protocol version or schema mismatch produces a clear error and no changes
- [ ] Property-style test: two databases with random independent edits, synced, end up with identical rows
- [ ] Test: the same set of changes applied in different orders gives the same result
- [ ] Test: a session cut off halfway, followed by a new session, finishes correctly without duplicating anything
- [ ] Test: with three databases (A↔B, then B↔C), C receives A's changes
