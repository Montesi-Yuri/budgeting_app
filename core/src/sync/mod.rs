//! Sync (sync-design.md §3) without networking: two Devices exchange the
//! changes each one is missing.
//!
//! - [`protocol`]: the messages ([`Msg`]) and their CBOR encoding.
//! - [`Session`]: the protocol logic of one side, with no I/O. Send the
//!   Hello it starts with, feed it every message received, and send every
//!   message it returns, until [`Session::is_finished`].
//! - [`run`]: drives a [`Session`] over any blocking [`Transport`].
//!   [`memory::pair`] is an in-process transport; the network transport
//!   (Noise over TCP) comes with Pairing (ticket 14).
//! - [`registry`]: maps a received op's entity name back to its row type.
//!
//! Received changes are applied with [`Store::apply_remote`](crate::Store::apply_remote),
//! one DB transaction per batch, so a session cut off halfway keeps every
//! batch it fully received, and the next session's Hello continues from there.

mod apply;
pub mod memory;
pub mod protocol;
pub mod registry;
mod session;

pub(crate) use apply::apply_op;
pub use protocol::{decode, encode, ErrCode, Msg, SyncError, VersionVector, PROTOCOL_VERSION};
pub use session::{run, Session, SyncReport, Transport};
