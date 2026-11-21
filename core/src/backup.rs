//! Backup: a passphrase-encrypted file containing all the data
//! (sync-design.md §4).
//!
//! A Backup is the whole sync log (every op of every origin), serialised as
//! CBOR and encrypted with `age` in passphrase mode. It holds no `device`
//! rows and no keys, so after a restore on a new installation the peers must
//! be paired again, and the restoring Device keeps its own device id.
//!
//! Restoring applies the ops with [`Store::apply_remote`], the same apply
//! path a Sync uses (sync-design.md §1.4), in one DB transaction: an op
//! already in the log is skipped, a new one is added to the log, and the row
//! takes the change only if it is newer (by HLC) than its current version.
//! So restoring only adds what is missing, and restoring twice changes
//! nothing the second time. Every entity in the Backup must be registered in
//! [`SYNCED_ENTITIES`](crate::sync::registry::SYNCED_ENTITIES); new synced
//! tables need no code here.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use age::secrecy::SecretString;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::SCHEMA_VERSION;
use crate::error::{Error, Result};
use crate::store::{Op, OpKind, Store};
use crate::sync::registry;

/// Written at the start of the decrypted content, to recognise a Backup.
const MAGIC: &str = "budget-backup";
const FORMAT: u32 = 1;

/// The outcome of a restore: how many changes were new to this database and
/// how many it already had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub added: u64,
    pub already_present: u64,
}

#[derive(Serialize, Deserialize)]
struct BackupFile {
    magic: String,
    format: u32,
    /// The schema (migration number) of the database it was made from.
    schema: u32,
    ops: Vec<BackupOp>,
}

#[derive(Serialize, Deserialize)]
struct BackupOp {
    origin: String,
    seq: u64,
    hlc: String,
    entity: String,
    entity_id: String,
    kind: String,
    #[serde(with = "serde_bytes")]
    payload: Option<Vec<u8>>,
}

/// The file name of a Backup made on `date` (`YYYY-MM-DD`):
/// `budget-YYYY-MM-DD.bsync.age`.
pub fn file_name(date: &str) -> String {
    format!("budget-{date}.bsync.age")
}

/// Write a Backup of all the data to `path`, encrypted with `passphrase`.
/// The file is written next to `path` first and then moved into place, so a
/// failure never leaves half a Backup.
pub fn export(store: &Store, path: &Path, passphrase: &str) -> Result<()> {
    if passphrase.is_empty() {
        return Err(Error::invalid("A Backup needs a passphrase."));
    }
    let mut ops = Vec::new();
    for origin in store.version_vector()?.into_keys() {
        for op in store.ops_since(&origin, 0)? {
            ops.push(BackupOp {
                origin: op.origin,
                seq: op.seq,
                hlc: op.hlc.to_string(),
                entity: op.entity,
                entity_id: op.entity_id,
                kind: op.kind.as_str().to_owned(),
                payload: op.payload,
            });
        }
    }
    let content = BackupFile {
        magic: MAGIC.to_owned(),
        format: FORMAT,
        schema: SCHEMA_VERSION,
        ops,
    };
    let mut plain = Vec::new();
    ciborium::into_writer(&content, &mut plain).map_err(|e| Error::Codec(e.to_string()))?;

    let encryptor = age::Encryptor::with_user_passphrase(SecretString::from(passphrase.to_owned()));
    let mut encrypted = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| Error::Codec(e.to_string()))?;
    writer.write_all(&plain)?;
    writer.finish()?;

    let partial = partial_path(path);
    std::fs::write(&partial, &encrypted)?;
    if let Err(err) = std::fs::rename(&partial, path) {
        let _ = std::fs::remove_file(&partial);
        return Err(err.into());
    }
    Ok(())
}

fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}

/// Apply every change in the Backup at `path` to this database, in one DB
/// transaction, through [`Store::apply_remote`]. A wrong passphrase, a file
/// that isn't a Backup, a Backup from a newer schema, or a change to an
/// entity this build doesn't know is an error and changes nothing.
pub fn restore(store: &mut Store, path: &Path, passphrase: &str) -> Result<RestoreReport> {
    let content = read(path, passphrase)?;
    let damaged = || Error::NotABackup("a change in it is damaged".into());
    let ops = content
        .ops
        .into_iter()
        .map(|op| {
            // Refuse the whole Backup before applying anything.
            if registry::lookup(&op.entity).is_err() {
                return Err(Error::NotABackup(format!(
                    "it contains changes to unknown data {:?}",
                    op.entity
                )));
            }
            Ok(Op {
                origin: op.origin,
                seq: op.seq,
                hlc: op.hlc.parse().map_err(|_| damaged())?,
                entity: op.entity,
                entity_id: op.entity_id,
                kind: OpKind::parse(&op.kind).map_err(|_| damaged())?,
                payload: op.payload,
            })
        })
        .collect::<Result<Vec<Op>>>()?;
    let applied = store.apply_remote(&ops).map_err(|err| match err {
        Error::Codec(_) => damaged(),
        other => other,
    })?;
    let added = applied.new_ops as u64;
    Ok(RestoreReport {
        added,
        already_present: ops.len() as u64 - added,
    })
}

fn read(path: &Path, passphrase: &str) -> Result<BackupFile> {
    let encrypted = std::fs::read(path)?;
    let decryptor = age::Decryptor::new(&encrypted[..])
        .map_err(|_| Error::NotABackup("it is not an encrypted Backup file".into()))?;
    if !decryptor.is_scrypt() {
        return Err(Error::NotABackup(
            "it is not protected by a passphrase".into(),
        ));
    }
    let identity = age::scrypt::Identity::new(SecretString::from(passphrase.to_owned()));
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|err| match err {
            age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
                Error::WrongPassphrase
            }
            other => Error::NotABackup(other.to_string()),
        })?;
    let mut plain = Vec::new();
    reader
        .read_to_end(&mut plain)
        .map_err(|_| Error::NotABackup("the file is damaged".into()))?;
    let content: BackupFile = ciborium::from_reader(&plain[..])
        .map_err(|_| Error::NotABackup("its content is not recognised".into()))?;
    if content.magic != MAGIC || content.format != FORMAT {
        return Err(Error::NotABackup("its content is not recognised".into()));
    }
    if content.schema > SCHEMA_VERSION {
        return Err(Error::SchemaTooNew {
            found: content.schema,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(content)
}

// ===== Weekly automatic Backup (PC) =====

/// The per-device setting holding the weekly Backup folder.
const SCHEDULE_FOLDER: &str = "backup_dir";
/// How many automatic Backups are kept in the folder.
pub const KEEP: usize = 8;
const WEEK_DAYS: i64 = 7;

/// The folder this Device writes its weekly automatic Backup to, if any.
/// It is a setting of this Device only: not synced, not in a Backup.
pub fn schedule_folder(store: &Store) -> Result<Option<PathBuf>> {
    let value: Option<String> = store
        .conn()
        .query_row(
            "SELECT value FROM local_setting WHERE key = ?1",
            [SCHEDULE_FOLDER],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|v| serde_json::from_str::<PathBuf>(&v).map_err(|e| Error::Codec(e.to_string())))
        .transpose()
}

/// Choose the weekly Backup folder, or turn the weekly Backup off with `None`.
pub fn set_schedule_folder(store: &mut Store, folder: Option<&Path>) -> Result<()> {
    // `local_setting` is not a synced table, so it is written directly.
    match folder {
        Some(folder) => {
            let value = serde_json::to_string(folder).map_err(|e| Error::Codec(e.to_string()))?;
            store.conn().execute(
                "INSERT INTO local_setting (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![SCHEDULE_FOLDER, value],
            )?;
        }
        None => {
            store.conn().execute(
                "DELETE FROM local_setting WHERE key = ?1",
                [SCHEDULE_FOLDER],
            )?;
        }
    }
    Ok(())
}

/// Write the weekly Backup to `folder` if the newest one there is at least a
/// week older than `today` (`YYYY-MM-DD`), then keep only the last [`KEEP`].
/// Returns the file written, or `None` if this week's Backup already exists.
/// Only files named like [`file_name`] are counted or removed.
pub fn run_weekly(
    store: &Store,
    folder: &Path,
    passphrase: &str,
    today: &str,
) -> Result<Option<PathBuf>> {
    let today_days = day_number(today).ok_or_else(|| Error::invalid("Not a valid date."))?;
    let mut existing = dated_backups(folder)?;
    if let Some((newest, _)) = existing.last() {
        if today_days - newest < WEEK_DAYS {
            return Ok(None);
        }
    }
    let path = folder.join(file_name(today));
    export(store, &path, passphrase)?;
    existing.push((today_days, path.clone()));
    existing.sort();
    existing.dedup();
    let excess = existing.len().saturating_sub(KEEP);
    for (_, old) in existing.into_iter().take(excess) {
        std::fs::remove_file(old)?;
    }
    Ok(Some(path))
}

/// The newest automatic Backup in `folder`, by the date in its name.
pub fn latest_weekly(folder: &Path) -> Result<Option<PathBuf>> {
    Ok(dated_backups(folder)?.pop().map(|(_, path)| path))
}

/// Today's date in UTC, `YYYY-MM-DD`.
pub fn today_utc() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    utc_date(now_ms)
}

/// The UTC date (`YYYY-MM-DD`) of a Unix time in milliseconds.
pub fn utc_date(unix_ms: u64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = (unix_ms / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The automatic Backups in `folder`, oldest first, by the date in their name.
fn dated_backups(folder: &Path) -> Result<Vec<(i64, PathBuf)>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(date) = name
            .to_str()
            .and_then(|n| n.strip_prefix("budget-"))
            .and_then(|n| n.strip_suffix(".bsync.age"))
        else {
            continue;
        };
        if let Some(day) = day_number(date) {
            found.push((day, entry.path()));
        }
    }
    found.sort();
    Ok(found)
}

/// Days since 1970-01-01 for a valid `YYYY-MM-DD` date.
fn day_number(date: &str) -> Option<i64> {
    if !crate::date::is_valid(date) {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (
        date[0..4].parse().ok()?,
        date[5..7].parse().ok()?,
        date[8..10].parse().ok()?,
    );
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}
