//! Synced settings: key/value rows in the `setting` table, values as JSON.
//! Per-device settings (such as a backup folder) do not belong here.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow};

const CURRENCY: &str = "currency";

/// One synced setting row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    /// JSON text.
    pub value: String,
}

impl SyncedRow for Setting {
    const ENTITY: &'static str = "setting";
    const ID_COLUMN: &'static str = "key";

    fn id(&self) -> &str {
        &self.key
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO setting (key, value, updated_hlc, deleted) VALUES (?1, ?2, ?3, 0)
             ON CONFLICT(key) DO UPDATE SET
               value = excluded.value, updated_hlc = excluded.updated_hlc, deleted = 0",
            params![self.key, self.value, hlc.to_string()],
        )?;
        Ok(())
    }
}

fn get<T: serde::de::DeserializeOwned>(store: &Store, key: &str) -> Result<Option<T>> {
    let value: Option<String> = store
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE key = ?1 AND deleted = 0",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|v| serde_json::from_str(&v).map_err(|e| Error::Codec(e.to_string())))
        .transpose()
}

fn set<T: Serialize>(store: &mut Store, key: &str, value: &T) -> Result<()> {
    let value = serde_json::to_string(value).map_err(|e| Error::Codec(e.to_string()))?;
    store.write(|w| {
        w.upsert(&Setting {
            key: key.to_owned(),
            value,
        })
    })?;
    Ok(())
}

/// The single Currency every amount is expressed in, as an ISO 4217 code
/// such as "EUR". `None` until the user chooses one on first launch.
pub fn currency(store: &Store) -> Result<Option<String>> {
    get(store, CURRENCY)
}

/// Set the Currency. `code` is an ISO 4217 code; it is stored upper case.
pub fn set_currency(store: &mut Store, code: &str) -> Result<()> {
    let code = code.trim().to_ascii_uppercase();
    if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(Error::invalid(
            "The Currency must be a three-letter code such as EUR.",
        ));
    }
    set(store, CURRENCY, &code)
}
