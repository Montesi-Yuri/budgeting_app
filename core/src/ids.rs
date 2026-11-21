//! Deterministic ids (UUIDv5) for rows that two Devices may create on their
//! own for the same thing, such as an imported Transaction (sync-design §1.5).
//! Both Devices compute the same id, so Sync merges the two rows into one.

use uuid::Uuid;

/// The app's UUIDv5 namespace. Never change it: every derived id depends on it.
pub const APP_NAMESPACE: Uuid = Uuid::from_u128(0x6f1d3b52_8c4e_4c8a_9a51_3e2f7b9d0c14);

/// The UUIDv5 of `name` in [`APP_NAMESPACE`], as text.
pub fn derived(name: &str) -> String {
    Uuid::new_v5(&APP_NAMESPACE, name.as_bytes()).to_string()
}
