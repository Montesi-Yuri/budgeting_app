//! Opening the SQLite database and running migrations.
//!
//! Migrations are plain SQL files in `core/migrations/`, applied in order.
//! The number of applied migrations is kept in SQLite's `user_version`; it is
//! also the `schema` number a Device announces in a sync Hello (§3.3).
//! Never edit a released migration: add a new file to the end of [`MIGRATIONS`].

use rusqlite::Connection;

use crate::error::{Error, Result};

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_categories_and_transactions.sql"),
    // 0003 and 0004 are reserved for tickets 13 and 12.
    include_str!("../migrations/0005_imports.sql"),
];

/// The schema version this build of the app creates and understands.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

pub(crate) fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    Ok(())
}

/// Bring the schema up to [`SCHEMA_VERSION`]. Each migration runs in its own
/// transaction together with the `user_version` bump.
pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(Error::SchemaTooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

pub(crate) fn schema_version(conn: &Connection) -> Result<u32> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}
