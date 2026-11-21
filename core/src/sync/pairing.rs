//! Pairing (sync-design.md §2): the payload the listening Device shows, and
//! its one-time token.
//!
//! The payload is a text, `bsync:1:<base64url(CBOR)>`, small enough for a QR
//! code (ticket 15). For now the other Device pastes it.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// How long a pairing token can be used: 5 minutes.
pub const PAIRING_TTL_SECS: u64 = 5 * 60;

const PREFIX: &str = "bsync:1:";
const APP: &str = "budget-sync";

/// Why Pairing failed. The messages are shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairingError {
    #[error("This is not a pairing code from the app: {0}")]
    BadPayload(String),
    #[error("The pairing code has expired. Start \"Pair new device\" again on the other Device.")]
    Expired,
    #[error("The pairing code is wrong or was already used. Start \"Pair new device\" again on the other Device.")]
    InvalidToken,
    #[error("Could not reach the other Device at {0}. Check that both Devices are on the same network and that Sync is on there.")]
    Unreachable(String),
    #[error("The other Device closed the connection: it does not know this Device. Pair the two Devices again.")]
    NotAccepted,
    #[error("This Device is not paired with {0}.")]
    UnknownPeer(String),
    #[error("Pairing is only possible while Sync is on.")]
    NotListening,
}

/// What the listening Device shows for "Pair new device" (§2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPayload {
    /// The listener's device id.
    pub device_id: String,
    pub name: String,
    /// The listener's IPv4 addresses, tried in order.
    pub addrs: Vec<String>,
    pub port: u16,
    /// The listener's Noise static public key.
    pub public_key: [u8; 32],
    /// The one-time pairing token.
    pub token: [u8; 32],
    /// Unix seconds after which the token is refused.
    pub expires_at: u64,
}

/// The CBOR form, with the short keys of §2.1.
#[derive(Serialize, Deserialize)]
struct Wire {
    v: u8,
    app: String,
    id: String,
    name: String,
    addrs: Vec<String>,
    port: u16,
    #[serde(with = "serde_bytes")]
    pk: Vec<u8>,
    #[serde(with = "serde_bytes")]
    tok: Vec<u8>,
    exp: u64,
}

impl PairingPayload {
    /// The text form, `bsync:1:<base64url(CBOR)>`.
    pub fn to_text(&self) -> String {
        let wire = Wire {
            v: 1,
            app: APP.to_owned(),
            id: self.device_id.clone(),
            name: self.name.clone(),
            addrs: self.addrs.clone(),
            port: self.port,
            pk: self.public_key.to_vec(),
            tok: self.token.to_vec(),
            exp: self.expires_at,
        };
        let mut bytes = Vec::new();
        ciborium::into_writer(&wire, &mut bytes).expect("CBOR into a Vec can't fail");
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
    }

    /// Read a pasted (or scanned) pairing payload.
    pub fn parse(text: &str) -> Result<PairingPayload> {
        let bad = |why: &str| Error::Pairing(PairingError::BadPayload(why.to_owned()));
        let body = text
            .trim()
            .strip_prefix(PREFIX)
            .ok_or_else(|| bad("it should start with \"bsync:1:\""))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| bad("it is damaged or incomplete"))?;
        let wire: Wire =
            ciborium::from_reader(&bytes[..]).map_err(|_| bad("it is damaged or incomplete"))?;
        if wire.v != 1 || wire.app != APP {
            return Err(bad("it is for another app or version"));
        }
        let key = |v: Vec<u8>| -> Result<[u8; 32]> {
            v.try_into().map_err(|_| bad("it is damaged or incomplete"))
        };
        if wire.addrs.is_empty() {
            return Err(bad("it has no address"));
        }
        Ok(PairingPayload {
            device_id: wire.id,
            name: wire.name,
            addrs: wire.addrs,
            port: wire.port,
            public_key: key(wire.pk)?,
            token: key(wire.tok)?,
            expires_at: wire.exp,
        })
    }
}

/// A pairing started on the listener and not used yet.
#[derive(Debug, Clone)]
pub(crate) struct PendingPairing {
    pub token: [u8; 32],
    pub expires_at: u64,
}

impl PendingPairing {
    /// Check a token a joining Device sent, `now` in Unix seconds.
    pub fn check(&self, token: &[u8], now: u64) -> std::result::Result<(), PairingError> {
        if !constant_time_eq(&self.token, token) {
            return Err(PairingError::InvalidToken);
        }
        if now >= self.expires_at {
            return Err(PairingError::Expired);
        }
        Ok(())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 32 random bytes from the OS.
pub(crate) fn random_bytes() -> Result<[u8; 32]> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| Error::Net(format!("no random numbers: {e}")))?;
    Ok(bytes)
}

/// This computer's private IPv4 addresses, for the pairing payload. Falls
/// back to the loopback address when there is no network (then only another
/// instance on the same computer can pair).
pub fn local_addresses() -> Vec<String> {
    let mut addrs: Vec<String> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|iface| match iface.ip() {
            std::net::IpAddr::V4(ip) if ip.is_private() && iface.is_oper_up() => {
                Some(ip.to_string())
            }
            _ => None,
        })
        .collect();
    addrs.sort();
    addrs.dedup();
    if addrs.is_empty() {
        addrs.push("127.0.0.1".to_owned());
    }
    addrs
}
