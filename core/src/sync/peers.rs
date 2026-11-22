//! Paired Devices: the `device` rows other than this Device's own. They are
//! per-Device data, not synced, so they are written with plain SQL here
//! rather than through the write path.

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::store::Store;

/// A Device this one is paired with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    /// Where it was last reached, "192.168.1.20:47321". `None` for a
    /// Device that connects to this one (it does not listen itself).
    pub last_addr: Option<String>,
    /// Unix seconds.
    pub paired_at: Option<i64>,
    /// Unix seconds of the last finished Sync with it.
    pub last_sync_at: Option<i64>,
}

const COLUMNS: &str = "device_id, name, last_addr, paired_at, last_sync_at";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Peer> {
    Ok(Peer {
        device_id: r.get(0)?,
        name: r.get(1)?,
        last_addr: r.get(2)?,
        paired_at: r.get(3)?,
        last_sync_at: r.get(4)?,
    })
}

/// Every paired Device, by name.
pub fn list(store: &Store) -> Result<Vec<Peer>> {
    let mut stmt = store.conn().prepare(&format!(
        "SELECT {COLUMNS} FROM device WHERE is_self = 0 AND static_pubkey IS NOT NULL
         ORDER BY name, device_id"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Unpair: forget the Device, so its key is refused from now on.
pub fn remove(store: &Store, device_id: &str) -> Result<()> {
    let n = store.conn().execute(
        "DELETE FROM device WHERE device_id = ?1 AND is_self = 0",
        [device_id],
    )?;
    if n == 0 {
        return Err(Error::NotFound {
            entity: "device",
            id: device_id.to_owned(),
        });
    }
    Ok(())
}

/// The paired Device and its public key.
pub(crate) fn get(store: &Store, device_id: &str) -> Result<Option<(Peer, [u8; 32])>> {
    store
        .conn()
        .query_row(
            &format!(
                "SELECT {COLUMNS}, static_pubkey FROM device
                 WHERE device_id = ?1 AND is_self = 0 AND static_pubkey IS NOT NULL"
            ),
            [device_id],
            |r| Ok((from_row(r)?, r.get::<_, Vec<u8>>(5)?)),
        )
        .optional()?
        .map(|(peer, key)| Ok((peer, key_from(key)?)))
        .transpose()
}

/// The paired Device with this Noise static key, if any.
pub(crate) fn find_by_key(store: &Store, key: &[u8; 32]) -> Result<Option<Peer>> {
    Ok(store
        .conn()
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM device WHERE is_self = 0 AND static_pubkey = ?1
                 ORDER BY paired_at DESC LIMIT 1"
            ),
            [&key[..]],
            from_row,
        )
        .optional()?)
}

/// Store (or refresh) a paired Device. A `None` address keeps the old one.
pub(crate) fn save(
    store: &Store,
    device_id: &str,
    name: &str,
    key: &[u8; 32],
    last_addr: Option<&str>,
    now: u64,
) -> Result<Peer> {
    if device_id == store.device_id() {
        return Err(Error::invalid("A Device can't be paired with itself."));
    }
    store.conn().execute(
        "INSERT INTO device (device_id, name, static_pubkey, is_self, last_addr, paired_at)
         VALUES (?1, ?2, ?3, 0, ?4, ?5)
         ON CONFLICT(device_id) DO UPDATE SET
           name = excluded.name, static_pubkey = excluded.static_pubkey,
           last_addr = COALESCE(excluded.last_addr, last_addr), paired_at = excluded.paired_at",
        params![device_id, name, &key[..], last_addr, now as i64],
    )?;
    Ok(get(store, device_id)?.expect("just saved").0)
}

/// Record a finished Sync with the Device.
pub(crate) fn touch_sync(store: &Store, device_id: &str, now: u64) -> Result<()> {
    store.conn().execute(
        "UPDATE device SET last_sync_at = ?2 WHERE device_id = ?1 AND is_self = 0",
        params![device_id, now as i64],
    )?;
    Ok(())
}

/// Record where the Device was reached.
pub(crate) fn set_addr(store: &Store, device_id: &str, addr: &str) -> Result<()> {
    store.conn().execute(
        "UPDATE device SET last_addr = ?2 WHERE device_id = ?1 AND is_self = 0",
        params![device_id, addr],
    )?;
    Ok(())
}

/// Put this Device's public key and read its name.
pub(crate) fn set_self_key(store: &Store, key: &[u8; 32]) -> Result<String> {
    store.conn().execute(
        "UPDATE device SET static_pubkey = ?1 WHERE is_self = 1",
        [&key[..]],
    )?;
    Ok(store
        .conn()
        .query_row("SELECT name FROM device WHERE is_self = 1", [], |r| {
            r.get(0)
        })?)
}

fn key_from(bytes: Vec<u8>) -> Result<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|_| Error::Codec("a stored Noise key is not 32 bytes".to_owned()))
}
