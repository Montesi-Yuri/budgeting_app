//! Pairing and Sync commands (sync-design.md §5). All the networking lives
//! in `budget_core::sync::SyncNode`; this module only keeps the per-Device
//! Sync settings, starts and stops the listener, and forwards the node's
//! events to the UI:
//!
//! - `sync://progress` — a [`SyncProgress`] while a Sync runs
//! - `sync://paired` — a [`Peer`] was paired (on either side)
//! - `sync://finished` — the [`SyncReport`] of a finished Sync
//! - `sync://failed` — a Sync or Pairing the other Device started failed
//! - `data://changed` — `{ entities }` after a Sync received changes
//!
//! Windows asks to allow the app through the firewall the first time it
//! listens on the network. Ticket 16 adds a rule for private networks.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use budget_core::sync::{
    local_addresses, Identity, Listener, Peer, SyncEvents, SyncNode, SyncProgress, SyncReport,
    DEFAULT_PORT,
};
use budget_core::Store;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::commands::{CmdResult, CommandError};

/// The per-Device Sync settings file in the app data folder. Not synced.
const SETTINGS_FILE: &str = "sync.json";
/// This Device's Noise key pair, in the app data folder.
const KEY_FILE: &str = "sync.key";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SyncSettings {
    /// Listen for other Devices ("Allow sync").
    pub enabled: bool,
    pub port: u16,
}

impl Default for SyncSettings {
    fn default() -> Self {
        SyncSettings {
            enabled: false,
            port: DEFAULT_PORT,
        }
    }
}

pub struct SyncState {
    node: SyncNode,
    settings_path: PathBuf,
    settings: Mutex<SyncSettings>,
    listener: Mutex<Option<Listener>>,
}

struct UiEvents(AppHandle);

#[derive(Clone, Serialize)]
struct DataChanged<'a> {
    entities: Vec<&'a str>,
}

impl SyncEvents for UiEvents {
    fn progress(&self, progress: &SyncProgress) {
        let _ = self.0.emit("sync://progress", progress);
    }
    fn paired(&self, peer: &Peer) {
        let _ = self.0.emit("sync://paired", peer);
    }
    fn synced(&self, report: &SyncReport) {
        let _ = self.0.emit("sync://finished", report);
        if report.received > 0 {
            let entities = report.entities.iter().map(String::as_str).collect();
            let _ = self.0.emit("data://changed", DataChanged { entities });
        }
    }
    fn failed(&self, error: &budget_core::Error) {
        let _ = self.0.emit("sync://failed", error.to_string());
    }
}

/// Set up Sync for the app: load the key and the settings, and start
/// listening if Sync was on.
pub fn init(
    app: &AppHandle,
    store: Arc<Mutex<Store>>,
    dir: PathBuf,
) -> budget_core::Result<SyncState> {
    let identity = Identity::load_or_create(dir.join(KEY_FILE))?;
    let node = SyncNode::with_options(
        store,
        identity,
        Box::new(UiEvents(app.clone())),
        Box::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }),
    )?;
    let settings_path = dir.join(SETTINGS_FILE);
    let settings: SyncSettings = std::fs::read(&settings_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let state = SyncState {
        node,
        settings_path,
        settings: Mutex::new(settings),
        listener: Mutex::new(None),
    };
    if settings.enabled {
        // If the port is taken, Sync stays off; the Settings page says so.
        let _ = state.apply(settings);
    }
    Ok(state)
}

impl SyncState {
    /// Start or stop the listener to match `settings`.
    fn apply(&self, settings: SyncSettings) -> CmdResult<()> {
        let mut listener = self.listener.lock().map_err(|_| unavailable())?;
        let wanted = settings.enabled.then_some(settings.port);
        if listener.as_ref().map(|l| l.port()) == wanted {
            return Ok(());
        }
        if let Some(old) = listener.take() {
            old.stop();
        }
        if let Some(port) = wanted {
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            *listener = Some(self.node.listen(addr)?);
        }
        Ok(())
    }

    fn save(&self, settings: SyncSettings) -> CmdResult<()> {
        let json = serde_json::to_vec_pretty(&settings).expect("settings serialize");
        std::fs::write(&self.settings_path, json).map_err(|e| {
            CommandError::new("internal", format!("could not save the Sync settings: {e}"))
        })?;
        *self.settings.lock().map_err(|_| unavailable())? = settings;
        Ok(())
    }
}

fn unavailable() -> CommandError {
    CommandError::new("internal", "Sync is unavailable")
}

#[derive(Debug, Serialize)]
pub struct SyncStatus {
    enabled: bool,
    port: u16,
    /// Whether the listener is actually running.
    listening: bool,
    /// This Device's addresses, as put in a pairing payload.
    addrs: Vec<String>,
}

#[tauri::command]
pub fn sync_status(sync: State<'_, SyncState>) -> CmdResult<SyncStatus> {
    let settings = *sync.settings.lock().map_err(|_| unavailable())?;
    let listening = sync.listener.lock().map_err(|_| unavailable())?.is_some();
    Ok(SyncStatus {
        enabled: settings.enabled,
        port: settings.port,
        listening,
        addrs: local_addresses(),
    })
}

/// Turn Sync (the listener) on or off, on `port`.
#[tauri::command]
pub fn sync_server_set(
    sync: State<'_, SyncState>,
    enabled: bool,
    port: u16,
) -> CmdResult<SyncStatus> {
    if port == 0 {
        return Err(CommandError::new(
            "invalid",
            "Choose a port between 1 and 65535.",
        ));
    }
    let settings = SyncSettings { enabled, port };
    sync.apply(settings)?;
    sync.save(settings)?;
    sync_status(sync)
}

#[derive(Debug, Serialize)]
pub struct PairingInfo {
    /// The text to paste on the other Device (later: shown as a QR code).
    payload: String,
    /// Unix seconds.
    expires_at: u64,
    addrs: Vec<String>,
    port: u16,
}

/// "Pair new device": a new one-time pairing payload, valid for 5 minutes.
#[tauri::command]
pub fn pairing_start(sync: State<'_, SyncState>) -> CmdResult<PairingInfo> {
    let payload = sync.node.start_pairing(local_addresses())?;
    Ok(PairingInfo {
        payload: payload.to_text(),
        expires_at: payload.expires_at,
        addrs: payload.addrs,
        port: payload.port,
    })
}

#[tauri::command]
pub fn pairing_cancel(sync: State<'_, SyncState>) {
    sync.node.cancel_pairing();
}

#[derive(Debug, Serialize)]
pub struct JoinResult {
    peer: Peer,
    report: SyncReport,
}

/// Pair with the Device whose payload was pasted, then run the first Sync.
#[tauri::command]
pub async fn pairing_join(sync: State<'_, SyncState>, payload: String) -> CmdResult<JoinResult> {
    let node = sync.node.clone();
    let joined = blocking(move || node.join(&payload)).await?;
    Ok(JoinResult {
        peer: joined.peer,
        report: joined.report,
    })
}

#[tauri::command]
pub fn peers_list(sync: State<'_, SyncState>) -> CmdResult<Vec<Peer>> {
    sync.node.peers().map_err(Into::into)
}

/// Unpair a Device: its later connections are refused.
#[tauri::command]
pub fn peer_remove(sync: State<'_, SyncState>, device_id: String) -> CmdResult<()> {
    sync.node.unpair(&device_id).map_err(Into::into)
}

/// "Sync now" with a paired Device.
#[tauri::command]
pub async fn sync_now(sync: State<'_, SyncState>, device_id: String) -> CmdResult<SyncReport> {
    let node = sync.node.clone();
    blocking(move || node.sync_with(&device_id)).await
}

/// Run blocking network code off the UI thread.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> budget_core::Result<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::new("internal", e.to_string()))?
        .map_err(Into::into)
}
