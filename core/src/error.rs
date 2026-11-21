use crate::hlc::ParseHlcError;

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
    /// The Backup's passphrase is not the one it was made with.
    #[error("The passphrase is wrong for this Backup. Nothing was changed.")]
    WrongPassphrase,
    /// The file is not a Backup this app can read.
    #[error("This file is not a Backup: {0}. Nothing was changed.")]
    NotABackup(String),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Error::Invalid(msg.into())
    }
}
