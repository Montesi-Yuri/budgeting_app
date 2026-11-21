use crate::hlc::ParseHlcError;
use crate::sync::{PairingError, SyncError};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The user's input can't be saved; the message is shown in the UI.
    #[error("{0}")]
    Invalid(String),
    #[error("{entity} {id} not found")]
    NotFound { entity: &'static str, id: String },
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("could not encode or decode a row: {0}")]
    Codec(String),
    #[error(transparent)]
    Hlc(#[from] ParseHlcError),
    #[error(
        "database schema {found} is newer than this app understands ({supported}); update the app"
    )]
    SchemaTooNew { found: u32, supported: u32 },
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    Pairing(#[from] PairingError),
    /// A network or encryption failure while talking to another Device.
    #[error("network error: {0}")]
    Net(String),
}

impl Error {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Error::Invalid(msg.into())
    }
}
