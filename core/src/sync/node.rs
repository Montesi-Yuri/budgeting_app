//! Pairing and Sync over the network (sync-design.md §2, §3): one
//! [`SyncNode`] per Device, shared by the listener and the commands.
//!
//! - The listening Device (the PC) runs a [`Listener`] while Sync is on.
//!   [`SyncNode::start_pairing`] gives the payload to show; the Device that
//!   joins passes it to [`SyncNode::join`]. Both then store each other as
//!   paired Devices, and the first Sync runs on the same connection.
//! - Later, [`SyncNode::sync_with`] connects to a paired Device's last known
//!   address and runs a Sync. The listener only accepts Devices it is paired
//!   with: any other key is disconnected during the handshake, before this
//!   Device sends anything.
//!
//! Everything is blocking: call it from a background thread. The Store is
//! locked once per message, not for a whole Sync, so the UI stays usable.

use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::store::Store;
use crate::sync::identity::Identity;
use crate::sync::pairing::{
    random_bytes, PairingError, PairingPayload, PendingPairing, PAIRING_TTL_SECS,
};
use crate::sync::peers::{self, Peer};
use crate::sync::protocol::{ErrCode, Msg, SyncError};
use crate::sync::secure::{self, SecureTransport};
use crate::sync::session::{Session, SyncReport, Transport};

/// The default TCP port the listener uses (§3.1).
pub const DEFAULT_PORT: u16 = 47321;

/// How long [`SyncNode::join`] and [`SyncNode::sync_with`] wait for a TCP
/// connection, per address, unless changed with [`SyncNode::set_connect_timeout`].
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Where a Sync is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    /// Hello exchanged, changes are being sent and received.
    Exchanging,
    /// Both sides sent Done.
    Finished,
}

/// Progress of a running Sync, for a progress indicator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncProgress {
    pub peer_device_id: String,
    pub phase: SyncPhase,
    /// Changes being sent to the other Device (known once its Hello arrived).
    pub sent: usize,
    /// New changes received so far.
    pub received: usize,
}

/// Notifications from a [`SyncNode`], also for Syncs and Pairings the other
/// Device started (on the listener). Called from background threads.
pub trait SyncEvents: Send + Sync {
    fn progress(&self, _progress: &SyncProgress) {}
    /// A Device was paired (on either side).
    fn paired(&self, _peer: &Peer) {}
    /// A Sync finished. If `report.received > 0`, the views showing
    /// `report.entities` should refresh.
    fn synced(&self, _report: &SyncReport) {}
    /// A Sync or Pairing started by the other Device failed on the listener.
    fn failed(&self, _error: &Error) {}
}

struct NoEvents;
impl SyncEvents for NoEvents {}

/// What [`SyncNode::join`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    /// The listening Device, now paired.
    pub peer: Peer,
    /// The first Sync with it.
    pub report: SyncReport,
}

type Clock = Box<dyn Fn() -> u64 + Send + Sync>;

struct Inner {
    store: Arc<Mutex<Store>>,
    identity: Identity,
    name: String,
    pending: Mutex<Option<PendingPairing>>,
    /// The port of the running listener, if any.
    listening: Mutex<Option<u16>>,
    events: Box<dyn SyncEvents>,
    now: Clock,
    connect_timeout: Mutex<Duration>,
}

/// Pairing and Sync for one Device. Cheap to clone (all clones share state).
#[derive(Clone)]
pub struct SyncNode {
    inner: Arc<Inner>,
}

impl SyncNode {
    /// A node for the Device that owns `store`, with its Noise key pair.
    /// Puts the public key into this Device's `device` row.
    pub fn new(store: Arc<Mutex<Store>>, identity: Identity) -> Result<SyncNode> {
        SyncNode::with_options(store, identity, Box::new(NoEvents), Box::new(unix_now))
    }

    /// Like [`SyncNode::new`], with an event sink and a clock (Unix
    /// seconds, for the pairing token's expiry).
    pub fn with_options(
        store: Arc<Mutex<Store>>,
        identity: Identity,
        events: Box<dyn SyncEvents>,
        now: Box<dyn Fn() -> u64 + Send + Sync>,
    ) -> Result<SyncNode> {
        let name = peers::set_self_key(&*lock(&store)?, identity.public_key())?;
        Ok(SyncNode {
            inner: Arc::new(Inner {
                store,
                identity,
                name,
                pending: Mutex::new(None),
                listening: Mutex::new(None),
                events,
                now,
                connect_timeout: Mutex::new(DEFAULT_CONNECT_TIMEOUT),
            }),
        })
    }

    /// How long to wait for a TCP connection per address (ticket 16 uses a
    /// short one for automatic Syncs).
    pub fn set_connect_timeout(&self, timeout: Duration) {
        *self
            .inner
            .connect_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = timeout;
    }

    /// Start accepting other Devices on `addr` (e.g. `0.0.0.0:47321`; port
    /// 0 picks a free one). Accepting stops when the [`Listener`] is dropped
    /// or [`Listener::stop`]ped. Only one listener per node.
    pub fn listen(&self, addr: SocketAddr) -> Result<Listener> {
        let socket = TcpListener::bind(addr)
            .map_err(|e| Error::Net(format!("could not listen on {addr}: {e}")))?;
        let local = socket.local_addr().map_err(|e| Error::Net(e.to_string()))?;
        *self
            .inner
            .listening
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(local.port());
        let stop = Arc::new(AtomicBool::new(false));
        let node = self.clone();
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            for stream in socket.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let node = node.clone();
                std::thread::spawn(move || {
                    if let Err(error) = node.serve(stream) {
                        node.inner.events.failed(&error);
                    }
                });
            }
        });
        Ok(Listener {
            node: self.clone(),
            addr: local,
            stop,
            thread: Some(thread),
        })
    }

    /// "Pair new device" on the listening Device: a new one-time token,
    /// valid for 5 minutes, replacing any earlier one. `addrs` are this
    /// Device's addresses to put in the payload (see
    /// [`local_addresses`](crate::sync::pairing::local_addresses)).
    pub fn start_pairing(&self, addrs: Vec<String>) -> Result<PairingPayload> {
        let port = self
            .inner
            .listening
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ok_or(PairingError::NotListening)?;
        let token = random_bytes()?;
        let expires_at = (self.inner.now)() + PAIRING_TTL_SECS;
        *self.pending() = Some(PendingPairing { token, expires_at });
        Ok(PairingPayload {
            device_id: lock(&self.inner.store)?.device_id().to_owned(),
            name: self.inner.name.clone(),
            addrs,
            port,
            public_key: *self.inner.identity.public_key(),
            token,
            expires_at,
        })
    }

    /// Forget the pending pairing token.
    pub fn cancel_pairing(&self) {
        self.pending().take();
    }

    /// On the joining Device: pair with the Device whose payload was
    /// pasted (or scanned), then run the first Sync. Tries each address in
    /// the payload in turn.
    pub fn join(&self, payload: &str) -> Result<Joined> {
        let payload = PairingPayload::parse(payload)?;
        let (mut conn, addr) = self.connect_any(
            payload
                .addrs
                .iter()
                .map(|a| format!("{a}:{}", payload.port)),
            &payload.public_key,
        )?;
        let device_id = lock(&self.inner.store)?.device_id().to_owned();
        conn.send(&Msg::PairRequest {
            token: payload.token.to_vec(),
            device_id,
            name: self.inner.name.clone(),
        })?;
        let name = match conn.recv()? {
            Msg::PairOk { name, .. } => name,
            Msg::Error { code, msg } => {
                return Err(match code {
                    ErrCode::PairingExpired => PairingError::Expired.into(),
                    ErrCode::PairingInvalid => PairingError::InvalidToken.into(),
                    code => SyncError::Refused { code, msg }.into(),
                })
            }
            _ => return Err(SyncError::Protocol("no answer to the pairing request".into()).into()),
        };
        let peer = peers::save(
            &*lock(&self.inner.store)?,
            &payload.device_id,
            &name,
            &payload.public_key,
            Some(&addr),
            (self.inner.now)(),
        )?;
        self.inner.events.paired(&peer);
        let report = self.exchange(&mut conn, &payload.device_id, None)?;
        conn.close();
        let peer = peers::get(&*lock(&self.inner.store)?, &payload.device_id)?
            .map(|(p, _)| p)
            .unwrap_or(peer);
        Ok(Joined { peer, report })
    }

    /// "Sync now": connect to the paired Device's last known address and run
    /// a Sync. The other Device must be listening (Sync on).
    pub fn sync_with(&self, device_id: &str) -> Result<SyncReport> {
        self.sync_with_addrs(device_id, &[])
    }

    /// Like [`SyncNode::sync_with`], also trying `extra_addrs` ("ip:port")
    /// after the last known address. The address that works is saved.
    pub fn sync_with_addrs(&self, device_id: &str, extra_addrs: &[String]) -> Result<SyncReport> {
        let (peer, key) = peers::get(&*lock(&self.inner.store)?, device_id)?
            .ok_or_else(|| PairingError::UnknownPeer(device_id.to_owned()))?;
        let addrs: Vec<String> = peer.last_addr.iter().chain(extra_addrs).cloned().collect();
        if addrs.is_empty() {
            return Err(Error::invalid(format!(
                "{} connects to this Device: start the Sync on {} instead.",
                peer.name, peer.name
            )));
        }
        let (mut conn, addr) = self.connect_any(addrs.into_iter(), &key)?;
        if peer.last_addr.as_deref() != Some(&addr) {
            peers::set_addr(&*lock(&self.inner.store)?, device_id, &addr)?;
        }
        let report = self.exchange(&mut conn, device_id, None)?;
        conn.close();
        Ok(report)
    }

    /// The paired Devices.
    pub fn peers(&self) -> Result<Vec<Peer>> {
        peers::list(&*lock(&self.inner.store)?)
    }

    /// Unpair: forget the Device; its later connections are refused.
    pub fn unpair(&self, device_id: &str) -> Result<()> {
        peers::remove(&*lock(&self.inner.store)?, device_id)
    }

    fn pending(&self) -> MutexGuard<'_, Option<PendingPairing>> {
        self.inner.pending.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Connect to the first address that completes the handshake with `key`.
    fn connect_any(
        &self,
        addrs: impl Iterator<Item = String>,
        key: &[u8; 32],
    ) -> Result<(SecureTransport, String)> {
        let timeout = *self
            .inner
            .connect_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut tried = Vec::new();
        let mut last_error = None;
        for addr in addrs {
            let resolved = addr.to_socket_addrs().ok().and_then(|mut a| a.next());
            let Some(socket_addr) = resolved else {
                tried.push(addr);
                continue;
            };
            match secure::connect(socket_addr, timeout, &self.inner.identity, key) {
                Ok(conn) => return Ok((conn, addr)),
                Err(Error::Pairing(PairingError::Unreachable(_))) => tried.push(addr),
                Err(e) => {
                    tried.push(addr);
                    last_error = Some(e);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| PairingError::Unreachable(tried.join(", ")).into()))
    }

    /// The listener's side of one connection.
    fn serve(&self, stream: TcpStream) -> Result<()> {
        let store = &self.inner.store;
        let pairing_open = self.pending().is_some();
        let mut known: Option<Peer> = None;
        let accepted = secure::accept(stream, &self.inner.identity, |key| {
            known = lock(store)
                .ok()
                .and_then(|s| peers::find_by_key(&s, key).ok().flatten());
            known.is_some() || pairing_open
        })?;
        let Some(mut conn) = accepted else {
            return Ok(());
        };
        match conn.recv()? {
            Msg::PairRequest {
                token,
                device_id,
                name,
            } => {
                let peer = match self.accept_pairing(&token, &device_id, &name, conn.remote_key()) {
                    Ok(peer) => peer,
                    Err(error) => {
                        if let Some(msg) = Msg::for_error(&error) {
                            let _ = conn.send(&msg);
                        }
                        conn.close();
                        return Err(error);
                    }
                };
                conn.send(&Msg::PairOk {
                    device_id: lock(store)?.device_id().to_owned(),
                    name: self.inner.name.clone(),
                })?;
                self.inner.events.paired(&peer);
                self.exchange(&mut conn, &peer.device_id, None)?;
            }
            hello @ Msg::Hello { .. } => {
                // Only a paired Device may sync.
                let Some(peer) = known else {
                    return Ok(());
                };
                self.exchange(&mut conn, &peer.device_id, Some(hello))?;
            }
            _ => {}
        }
        conn.close();
        Ok(())
    }

    /// Check the token and store the joining Device. The token is used up.
    fn accept_pairing(
        &self,
        token: &[u8],
        device_id: &str,
        name: &str,
        key: &[u8; 32],
    ) -> Result<Peer> {
        let mut pending = self.pending();
        let Some(open) = pending.as_ref() else {
            return Err(PairingError::InvalidToken.into());
        };
        open.check(token, (self.inner.now)())?;
        let peer = peers::save(
            &*lock(&self.inner.store)?,
            device_id,
            name,
            key,
            None,
            (self.inner.now)(),
        )?;
        *pending = None;
        Ok(peer)
    }

    /// Run a Sync session on an open connection. `first` is a message
    /// already received from the other side (its Hello).
    fn exchange(
        &self,
        conn: &mut SecureTransport,
        peer_device_id: &str,
        first: Option<Msg>,
    ) -> Result<SyncReport> {
        let store = &self.inner.store;
        let (mut session, hello) = Session::start(&*lock(store)?)?;
        conn.send(&hello)?;
        let mut first = first;
        while !session.is_finished() {
            let msg = match first.take() {
                Some(msg) => msg,
                None => conn.recv()?,
            };
            let result = session.handle(&mut *lock(store)?, msg);
            match result {
                Ok(replies) => {
                    for reply in &replies {
                        conn.send(reply)?;
                    }
                }
                Err(error) => {
                    if let Some(msg) = Msg::for_error(&error) {
                        let _ = conn.send(&msg);
                    }
                    return Err(error);
                }
            }
            let report = session.report();
            self.inner.events.progress(&SyncProgress {
                peer_device_id: peer_device_id.to_owned(),
                phase: if session.is_finished() {
                    SyncPhase::Finished
                } else {
                    SyncPhase::Exchanging
                },
                sent: report.sent,
                received: report.received,
            });
        }
        let report = session.into_report();
        if report.peer_device_id != peer_device_id {
            return Err(SyncError::Protocol(
                "the other Device sent a different device id than the one paired".into(),
            )
            .into());
        }
        peers::touch_sync(&*lock(store)?, peer_device_id, (self.inner.now)())?;
        self.inner.events.synced(&report);
        Ok(report)
    }
}

/// A running listener (Sync is on). Dropping it stops accepting new
/// connections; Syncs already running finish.
pub struct Listener {
    node: SyncNode,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// The address it is bound to (with the actual port when 0 was asked).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Stop accepting connections, and cancel a pending pairing.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking accept with a connection to ourselves.
        let wake: SocketAddr = if self.addr.ip().is_unspecified() {
            SocketAddr::from(([127, 0, 0, 1], self.addr.port()))
        } else {
            self.addr
        };
        let _ = TcpStream::connect_timeout(&wake, Duration::from_secs(1));
        let _ = thread.join();
        let inner = &self.node.inner;
        *inner.listening.lock().unwrap_or_else(|e| e.into_inner()) = None;
        inner
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn lock(store: &Mutex<Store>) -> Result<MutexGuard<'_, Store>> {
    store
        .lock()
        .map_err(|_| Error::Net("the database is unavailable".to_owned()))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
