//! Applying one received op inside the caller's transaction.
//!
//! A row's state is a function of the set of ops for it in the sync log, so
//! it doesn't matter in which order the ops arrive:
//!
//! - the newest op (by HLC) decides `updated_hlc` and whether it is deleted;
//! - the newest upsert decides the row's columns (for a tombstone too).
//!
//! The local write path keeps the same invariant, because a local write is
//! always stamped after every op the Device holds.

use rusqlite::{params, Connection};

use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Op, OpKind};
use crate::sync::registry::{self, SyncedEntity};

/// Record `op` in the sync log if it is new, and bring its row up to date.
/// Returns whether the op was new.
pub(crate) fn apply_op(conn: &Connection, op: &Op) -> Result<bool> {
    let entity = registry::lookup(&op.entity)?;
    if op.kind == OpKind::Upsert && op.payload.is_none() {
        return Err(Error::Codec(format!(
            "upsert {}#{} has no payload",
            op.origin, op.seq
        )));
    }
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO op (origin, seq, hlc, entity, entity_id, kind, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            op.origin,
            op.seq as i64,
            op.hlc.to_string(),
            op.entity,
            op.entity_id,
            op.kind.as_str(),
            op.payload
        ],
    )?;
    if inserted == 0 {
        return Ok(false);
    }

    let newest = newest_hlc(conn, op, None)?;
    match op.kind {
        OpKind::Upsert => {
            if newest_hlc(conn, op, Some(OpKind::Upsert))? == op.hlc {
                entity.save_payload(conn, op.payload.as_deref().unwrap_or_default(), &op.hlc)?;
                if newest != op.hlc {
                    // A newer delete is already here: the row stays a tombstone.
                    mark_deleted(conn, entity, &op.entity_id, &newest)?;
                }
            }
        }
        OpKind::Delete => {
            if newest == op.hlc {
                mark_deleted(conn, entity, &op.entity_id, &op.hlc)?;
            }
        }
    }
    Ok(true)
}

/// The newest HLC among the logged ops for `op`'s row, optionally of one kind.
fn newest_hlc(conn: &Connection, op: &Op, kind: Option<OpKind>) -> Result<Hlc> {
    let newest: String = conn
        .prepare_cached(
            "SELECT MAX(hlc) FROM op
             WHERE entity = ?1 AND entity_id = ?2 AND (?3 IS NULL OR kind = ?3)",
        )?
        .query_row(
            params![op.entity, op.entity_id, kind.map(OpKind::as_str)],
            |r| r.get(0),
        )?;
    Ok(newest.parse()?)
}

/// Soft-delete the row if it exists. If it doesn't yet, the upsert that
/// creates it will find this delete in the log when it arrives.
fn mark_deleted(conn: &Connection, entity: &SyncedEntity, id: &str, hlc: &Hlc) -> Result<()> {
    conn.execute(
        &format!(
            "UPDATE {table} SET deleted = 1, updated_hlc = ?1 WHERE {id_col} = ?2",
            table = entity.name,
            id_col = entity.id_column,
        ),
        params![hlc.to_string(), id],
    )?;
    Ok(())
}
