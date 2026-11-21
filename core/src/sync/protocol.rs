//! Sync messages (sync-design.md §3.1). Each message is one CBOR value, and
//! a transport sends it as one frame.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::store::Op;

/// The sync protocol version sent in [`Msg::Hello`]. Two Devices only sync
/// when they use the same version.
pub const PROTOCOL_VERSION: u16 = 1;

/// The largest encoded [`Msg::Ops`] a session sends, in bytes. It leaves
/// room for the Noise tag within Noise's 65,535-byte message limit.
pub const MAX_MESSAGE_BYTES: usize = 60 * 1024;

/// The most changes sent in one [`Msg::Ops`] batch.
pub const MAX_BATCH_OPS: usize = 200;

/// The highest `seq` a Device holds per origin device id: "I have every op
/// up to PC#120 and PHONE#45". See [`Store::version_vector`](crate::Store::version_vector).
pub type VersionVector = BTreeMap<String, u64>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Msg {
    /// The first message of each side: who I am and what I already have.
    Hello {
        proto: u16,
        device_id: String,
        /// The DB migration number ([`crate::SCHEMA_VERSION`]).
        schema: u32,
        vv: VersionVector,
    },
    /// A batch of the changes the other side is missing, ordered by
    /// (origin, seq). The last batch has `last: true` (it may be empty).
    Ops { ops: Vec<Op>, last: bool },
    /// Sent after applying the other side's last batch: my version vector now.
    Done { vv: VersionVector },
    /// The sender stops the session.
    Error { code: ErrCode, msg: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrCode {
    /// The two Devices use different sync protocol versions.
    IncompatibleProtocol,
    /// The receiver's database schema is newer than the sender's, so the
    /// sender can't apply its changes: the sender's app must be updated.
    UpdateApp,
    /// A message arrived that the protocol doesn't allow at that point.
    Protocol,
    /// Something went wrong on the sender's side, such as a database error.
    Internal,
}

/// Why a Sync session failed. The messages are shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    #[error("the other Device uses sync protocol version {theirs} and this one uses {mine}: update the app on the Device that is behind")]
    IncompatibleProtocol { mine: u16, theirs: u16 },
    #[error("the other Device has a newer version of the app (database schema {theirs}, this one has {mine}): update the app on this Device")]
    UpdateApp { mine: u32, theirs: u32 },
    /// The other Device sent [`Msg::Error`]; `msg` is its own wording.
    #[error("{}", refused_text(*.code, .msg))]
    Refused { code: ErrCode, msg: String },
    #[error("unexpected message during Sync: {0}")]
    Protocol(String),
    #[error("the connection to the other Device was lost during Sync")]
    Disconnected,
}

fn refused_text(code: ErrCode, msg: &str) -> String {
    match code {
        ErrCode::UpdateApp => {
            "the other Device has an older version of the app: update the app on the other Device"
                .to_owned()
        }
        ErrCode::IncompatibleProtocol => "the two Devices use different sync protocol versions: update the app on the Device that is behind".to_owned(),
        ErrCode::Protocol | ErrCode::Internal => format!("the other Device stopped the Sync: {msg}"),
    }
}

impl Msg {
    /// The [`Msg::Error`] to send the other side when this side's session
    /// fails with `error`, or `None` when there is no one to tell (the other
    /// side stopped the session, or the connection is gone).
    pub fn for_error(error: &Error) -> Option<Msg> {
        let code = match error {
            Error::Sync(SyncError::Refused { .. } | SyncError::Disconnected) => return None,
            Error::Sync(SyncError::IncompatibleProtocol { .. }) => ErrCode::IncompatibleProtocol,
            Error::Sync(SyncError::UpdateApp { .. }) => ErrCode::UpdateApp,
            Error::Sync(SyncError::Protocol(_)) => ErrCode::Protocol,
            _ => ErrCode::Internal,
        };
        Some(Msg::Error {
            code,
            msg: error.to_string(),
        })
    }
}

/// Encode a message as CBOR, for one transport frame.
pub fn encode(msg: &Msg) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    ciborium::into_writer(msg, &mut bytes).map_err(|e| Error::Codec(e.to_string()))?;
    Ok(bytes)
}

/// Decode one frame received from the other side.
pub fn decode(bytes: &[u8]) -> Result<Msg> {
    ciborium::from_reader(bytes).map_err(|e| Error::Codec(e.to_string()))
}
