//! This Device's Noise static key pair (X25519). The public key goes into
//! the pairing payload and the `device` row; the private key never leaves
//! the Device and is not in the database, so it is not in a Backup either.

use std::path::Path;

use crate::error::{Error, Result};
use crate::sync::secure::NOISE_PARAMS;

/// This Device's Noise static key pair.
#[derive(Clone)]
pub struct Identity {
    private: [u8; 32],
    public: [u8; 32],
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

impl Identity {
    /// A new random key pair.
    pub fn generate() -> Result<Identity> {
        let params = NOISE_PARAMS.parse().map_err(noise_error)?;
        let keypair = snow::Builder::new(params)
            .generate_keypair()
            .map_err(noise_error)?;
        Ok(Identity {
            private: to_key(&keypair.private)?,
            public: to_key(&keypair.public)?,
        })
    }

    /// The key pair kept in `path` (64 bytes: private, then public),
    /// created on first use. If the file is lost, a new key is made and the
    /// paired Devices must be paired again.
    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Identity> {
        let path = path.as_ref();
        match std::fs::read(path) {
            Ok(bytes) if bytes.len() == 64 => Ok(Identity {
                private: to_key(&bytes[..32])?,
                public: to_key(&bytes[32..])?,
            }),
            Ok(_) => Err(Error::Net(format!(
                "the sync key file {} is damaged; delete it and pair again",
                path.display()
            ))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let identity = Identity::generate()?;
                let mut bytes = identity.private.to_vec();
                bytes.extend_from_slice(&identity.public);
                std::fs::write(path, bytes).map_err(|e| {
                    Error::Net(format!(
                        "could not save the sync key {}: {e}",
                        path.display()
                    ))
                })?;
                Ok(identity)
            }
            Err(e) => Err(Error::Net(format!(
                "could not read the sync key {}: {e}",
                path.display()
            ))),
        }
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.public
    }

    pub(crate) fn private_key(&self) -> &[u8; 32] {
        &self.private
    }
}

fn to_key(bytes: &[u8]) -> Result<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|_| Error::Net("a Noise key is not 32 bytes".to_owned()))
}

pub(crate) fn noise_error(e: snow::Error) -> Error {
    Error::Net(format!("encryption error: {e}"))
}
