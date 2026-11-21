//! The Store: the local database and **the single write path**.
//!
//! Every change to synced data goes through [`Store::write`]. Inside it, each
//! [`Writer::upsert`] / [`Writer::delete`]:
//!
//! 1. stamps the change with the Device's hybrid logical clock ([`Hlc`]),
//! 2. writes the entity row with that stamp in `updated_hlc`
//!    (a delete is a soft delete: `deleted = 1`, the row stays as a tombstone),
//! 3. appends an [`Op`] to the sync log (`op` table) with the next `seq` for
//!    this Device and, for an upsert, the full row as CBOR.
//!
//! All the changes made inside one `write` closure commit in **one** DB
//! transaction, or none of them do (return `Err` from the closure to roll
//! back). Use one `write` call per user action, e.g. "mark this Account as
//! main" writes several Account rows atomically.
//!
//! Reads go straight to SQL through [`Store::conn`] (or [`Writer::conn`]
//! inside a write); remember to filter out `deleted = 1` rows.
//!
//! ## Adding a synced entity (later tickets)
//!
//! 1. Add a migration with the table, including `updated_hlc TEXT NOT NULL`
//!    and `deleted INTEGER NOT NULL DEFAULT 0`.
//! 2. Make a `Serialize + Deserialize` struct holding the full row (without
//!    `updated_hlc` / `deleted`) and implement [`SyncedRow`] for it.
//! 3. Write it only via `store.write(|w| w.upsert(&row))` / `w.delete::<Row>(id)`.
//!    Never `INSERT`/`UPDATE`/`DELETE` a synced table anywhere else.
//! 4. Register the entity name where remote ops are applied (ticket 13).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::db;
use crate::error::{Error, Result};
use crate::hlc::{Clock, Hlc};

/// A row of a synced table. The struct must hold every column except
/// `updated_hlc` and `deleted`, because its CBOR encoding is the op payload
/// another Device uses to rebuild the row.
pub trait SyncedRow: Serialize + DeserializeOwned {
    /// The entity name, written to `op.entity`; also the table name.
    const ENTITY: &'static str;
    /// The primary key column of the table.
    const ID_COLUMN: &'static str = "id";

    fn id(&self) -> &str;

    /// Insert or fully replace the row, setting `updated_hlc = hlc` and
    /// `deleted = 0`. Called only by the write path (and, later, the apply
    /// path for remote ops). Implementations use `INSERT ... ON CONFLICT DO UPDATE`.
    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Upsert,
    Delete,
}

impl OpKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            OpKind::Upsert => "upsert",
            OpKind::Delete => "delete",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self> {
        match s {
            "upsert" => Ok(OpKind::Upsert),
            "delete" => Ok(OpKind::Delete),
            other => Err(Error::Codec(format!("unknown op kind {other:?}"))),
        }
    }
}

/// One change in the sync log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    /// The device id of the Device that made the change.
    pub origin: String,
    /// 1, 2, 3, ... per origin, with no gaps.
    pub seq: u64,
    pub hlc: Hlc,
    pub entity: String,
    pub entity_id: String,
    pub kind: OpKind,
    /// CBOR of the full row, for upserts only.
    pub payload: Option<Vec<u8>>,
}

impl Op {
    /// Decode the row carried by an upsert. `Ok(None)` for a delete.
    pub fn decode_payload<R: DeserializeOwned>(&self) -> Result<Option<R>> {
        self.payload
            .as_deref()
            .map(|bytes| ciborium::from_reader(bytes).map_err(|e| Error::Codec(e.to_string())))
            .transpose()
    }
}

type WallClock = Box<dyn Fn() -> u64 + Send>;

fn system_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct Store {
    conn: Connection,
    device_id: String,
    clock: Clock,
    now_ms: WallClock,
}

impl Store {
    /// Open (creating it if needed) the database file, run migrations, and on
    /// first launch give this Device its permanent device id.
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        Store::init(Connection::open(path)?)
    }

    /// A fresh, private in-memory database. For tests.
    pub fn open_in_memory() -> Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Store> {
        db::configure(&conn)?;
        db::migrate(&mut conn)?;
        let device_id = ensure_self_device(&conn)?;
        let mut clock = Clock::new(&device_id);
        let latest: Option<String> = conn.query_row("SELECT MAX(hlc) FROM op", [], |r| r.get(0))?;
        if let Some(latest) = latest {
            clock.observe(&latest.parse()?);
        }
        Ok(Store {
            conn,
            device_id,
            clock,
            now_ms: Box::new(system_now_ms),
        })
    }

    /// Replace the wall clock the HLC reads (Unix ms). For tests.
    pub fn set_wall_clock(&mut self, now_ms: impl Fn() -> u64 + Send + 'static) {
        self.now_ms = Box::new(now_ms);
    }

    /// This Device's permanent id (UUIDv4), created on first launch.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// The migration number of the open database.
    pub fn schema_version(&self) -> Result<u32> {
        db::schema_version(&self.conn)
    }

    /// Read-only access for queries. Never write synced tables through this.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run `f` in one DB transaction. Every row it writes and every op it
    /// appends commit together; if `f` returns `Err`, nothing is kept.
    pub fn write<T>(&mut self, f: impl FnOnce(&mut Writer<'_>) -> Result<T>) -> Result<T> {
        let tx = self.conn.transaction()?;
        let mut writer = Writer {
            tx: &tx,
            clock: &mut self.clock,
            device_id: &self.device_id,
            now_ms: &self.now_ms,
        };
        let out = f(&mut writer)?;
        tx.commit()?;
        Ok(out)
    }

    /// For modules of this crate that apply ops made elsewhere (Backup
    /// restore): run `f` in one DB transaction, then let the clock observe
    /// the newest HLC it returns, so later local writes stamp newer changes.
    pub(crate) fn apply_foreign<T>(
        &mut self,
        f: impl FnOnce(&Transaction<'_>) -> Result<(T, Option<Hlc>)>,
    ) -> Result<T> {
        let tx = self.conn.transaction()?;
        let (out, newest) = f(&tx)?;
        tx.commit()?;
        if let Some(newest) = newest {
            self.clock.observe(&newest);
        }
        Ok(out)
    }

    /// The ops made by `origin` with `seq > after_seq`, in `seq` order.
    pub fn ops_since(&self, origin: &str, after_seq: u64) -> Result<Vec<Op>> {
        let mut stmt = self.conn.prepare(
            "SELECT origin, seq, hlc, entity, entity_id, kind, payload
             FROM op WHERE origin = ?1 AND seq > ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![origin, after_seq as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<Vec<u8>>>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (origin, seq, hlc, entity, entity_id, kind, payload) = row?;
            Ok(Op {
                origin,
                seq: seq as u64,
                hlc: hlc.parse()?,
                entity,
                entity_id,
                kind: OpKind::parse(&kind)?,
                payload,
            })
        })
        .collect()
    }

    /// The highest `seq` held per origin: "I have every op up to ...".
    pub fn version_vector(&self) -> Result<BTreeMap<String, u64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT origin, MAX(seq) FROM op GROUP BY origin")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

/// Handle given to the closure of [`Store::write`]. See the module docs.
pub struct Writer<'a> {
    tx: &'a Transaction<'a>,
    clock: &'a mut Clock,
    device_id: &'a str,
    now_ms: &'a WallClock,
}

impl Writer<'_> {
    /// Reads inside the write's transaction (they see its earlier writes).
    pub fn conn(&self) -> &Connection {
        self.tx
    }

    /// Create or replace a row, logging an upsert op with the full row.
    pub fn upsert<R: SyncedRow>(&mut self, row: &R) -> Result<Hlc> {
        let hlc = self.clock.tick((self.now_ms)());
        row.save(self.tx, &hlc)?;
        let mut payload = Vec::new();
        ciborium::into_writer(row, &mut payload).map_err(|e| Error::Codec(e.to_string()))?;
        self.append_op(&hlc, R::ENTITY, row.id(), OpKind::Upsert, Some(payload))?;
        Ok(hlc)
    }

    /// Soft-delete a row (`deleted = 1`), logging a delete op. Fails with
    /// [`Error::NotFound`] if there is no live row with that id.
    pub fn delete<R: SyncedRow>(&mut self, id: &str) -> Result<Hlc> {
        let hlc = self.clock.tick((self.now_ms)());
        let sql = format!(
            "UPDATE {table} SET deleted = 1, updated_hlc = ?1 WHERE {id_col} = ?2 AND deleted = 0",
            table = R::ENTITY,
            id_col = R::ID_COLUMN,
        );
        if self.tx.execute(&sql, params![hlc.to_string(), id])? == 0 {
            return Err(Error::NotFound {
                entity: R::ENTITY,
                id: id.to_owned(),
            });
        }
        self.append_op(&hlc, R::ENTITY, id, OpKind::Delete, None)?;
        Ok(hlc)
    }

    fn append_op(
        &self,
        hlc: &Hlc,
        entity: &str,
        entity_id: &str,
        kind: OpKind,
        payload: Option<Vec<u8>>,
    ) -> Result<()> {
        self.tx.execute(
            "INSERT INTO op (origin, seq, hlc, entity, entity_id, kind, payload)
             VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM op WHERE origin = ?1), ?2, ?3, ?4, ?5, ?6)",
            params![self.device_id, hlc.to_string(), entity, entity_id, kind.as_str(), payload],
        )?;
        Ok(())
    }
}

fn ensure_self_device(conn: &Connection) -> Result<String> {
    let existing: Option<String> = conn
        .query_row("SELECT device_id FROM device WHERE is_self = 1", [], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let name = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "This device".to_owned());
    conn.execute(
        "INSERT INTO device (device_id, name, is_self) VALUES (?1, ?2, 1)",
        params![id, name],
    )?;
    Ok(id)
}
