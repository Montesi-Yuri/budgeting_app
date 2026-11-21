//! The registry of synced entities: how a received op's `entity` name maps
//! back to its [`SyncedRow`] type, so the apply path can rebuild the row.
//!
//! **Every [`SyncedRow`] type must be listed in [`SYNCED_ENTITIES`]**, one line
//! per entity: `entity::<MyRow>(),`. A test fails if a table with an
//! `updated_hlc` column is not registered here.

use rusqlite::Connection;

use crate::accounts::Account;
use crate::budgets::Budget;
use crate::categories::Category;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::settings::Setting;
use crate::store::SyncedRow;
use crate::transactions::Transaction;

/// Every synced entity this build knows. Add new entities at the end.
pub const SYNCED_ENTITIES: &[SyncedEntity] = &[
    entity::<Setting>(),
    entity::<Account>(),
    entity::<Category>(),
    entity::<Transaction>(),
    entity::<Budget>(),
];

/// One registered synced entity: its table and how to save a row decoded
/// from an op payload.
#[derive(Clone, Copy)]
pub struct SyncedEntity {
    /// [`SyncedRow::ENTITY`]: the `op.entity` value and the table name.
    pub name: &'static str,
    /// [`SyncedRow::ID_COLUMN`].
    pub id_column: &'static str,
    save_payload: fn(&Connection, &[u8], &Hlc) -> Result<()>,
}

impl SyncedEntity {
    /// Decode an upsert payload and save it with `hlc` as its `updated_hlc`.
    pub(crate) fn save_payload(&self, conn: &Connection, payload: &[u8], hlc: &Hlc) -> Result<()> {
        (self.save_payload)(conn, payload, hlc)
    }
}

/// The registry entry for `R`.
pub const fn entity<R: SyncedRow>() -> SyncedEntity {
    SyncedEntity {
        name: R::ENTITY,
        id_column: R::ID_COLUMN,
        save_payload: save_payload::<R>,
    }
}

fn save_payload<R: SyncedRow>(conn: &Connection, payload: &[u8], hlc: &Hlc) -> Result<()> {
    let row: R = ciborium::from_reader(payload).map_err(|e| Error::Codec(e.to_string()))?;
    row.save(conn, hlc)?;
    Ok(())
}

/// The registered entity called `name`.
pub fn lookup(name: &str) -> Result<&'static SyncedEntity> {
    SYNCED_ENTITIES
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| Error::Codec(format!("received a change to an unknown entity {name:?}")))
}
