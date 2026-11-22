//! Sync (sync-design.md §3): two Devices exchange the changes each one is
//! missing, and Pairing (§2), over Noise-encrypted TCP.
//!
//! - [`protocol`]: the messages ([`Msg`]) and their CBOR encoding.
//! - [`Session`]: the protocol logic of one side, with no I/O. Send the
//!   Hello it starts with, feed it every message received, and send every
//!   message it returns, until [`Session::is_finished`].
//! - [`run`]: drives a [`Session`] over any blocking [`Transport`].
//!   [`memory::pair`] is an in-process transport.
//! - [`registry`]: maps a received op's entity name back to its row type.
//! - [`SyncNode`]: Pairing and Sync over the network for one Device: a
//!   [`Listener`] while Sync is on, [`SyncNode::start_pairing`] /
//!   [`SyncNode::join`] with a [`PairingPayload`], [`SyncNode::sync_with`]
//!   for "Sync now", and [`SyncEvents`] for progress and UI refreshes.
//!   The encrypted transport (Noise IK over TCP, `u16` length frames) is
//!   private to it. [`Identity`] is the Device's Noise key pair, and
//!   [`peers`] the paired Devices.
//! - [`discovery`]: the listening Device announces itself on the local
//!   network ([`Mdns`]) while it listens, and a Sync whose known addresses
//!   fail looks for the Device there ([`SyncNode::set_discovery`]).
//!   [`SyncNode::sync_automatically`] is the quick Sync when the phone app
//!   comes to the foreground.
//!
//! Received changes are applied with [`Store::apply_remote`](crate::Store::apply_remote),
//! one DB transaction per batch, so a session cut off halfway keeps every
//! batch it fully received, and the next session's Hello continues from there.

mod apply;
pub mod discovery;
mod identity;
pub mod memory;
mod node;
pub mod pairing;
pub mod peers;
pub mod protocol;
pub mod registry;
mod secure;
mod session;

pub(crate) use apply::apply_op;
pub use discovery::{Announcement, Discovery, Mdns};
pub use identity::Identity;
pub use node::{
    Joined, Listener, SyncEvents, SyncNode, SyncPhase, SyncProgress, AUTO_SYNC_TIMEOUT,
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_PORT,
};
pub use pairing::{local_addresses, PairingError, PairingPayload};
pub use peers::Peer;
pub use protocol::{decode, encode, ErrCode, Msg, SyncError, VersionVector, PROTOCOL_VERSION};
pub use session::{run, Session, SyncReport, Transport};
