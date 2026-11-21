//! One side of a Sync session (sync-design.md §3.2). Both sides run the same
//! logic at the same time:
//!
//! ```text
//! send Hello{vv}           ──▶   ◀── Hello{vv}
//! send what the peer lacks ──▶   ◀── its batches   (each applied in one DB transaction)
//! after its last batch: Done{vv'} ──▶   ◀── Done{vv'}
//! ```

use std::collections::BTreeSet;

use crate::error::{Error, Result};
use crate::store::{Op, Store};
use crate::sync::protocol::{
    Msg, SyncError, VersionVector, MAX_BATCH_OPS, MAX_MESSAGE_BYTES, PROTOCOL_VERSION,
};

/// Room left in each [`Msg::Ops`] for the message's own framing.
const OPS_MESSAGE_OVERHEAD: usize = 64;

/// What a finished (or interrupted) session did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// The other Device's device id, from its Hello.
    pub peer_device_id: String,
    /// Changes sent to the other Device.
    pub sent: usize,
    /// Changes received that this Device didn't have yet.
    pub received: usize,
    /// The entities the received changes touched, so the UI can refetch them.
    pub entities: BTreeSet<String>,
    /// Both Devices reported the same version vector in their Done: they
    /// hold the same changes. `false` if one of them changed data during the
    /// session (the next Sync picks that up).
    pub converged: bool,
}

/// A transport for [`run`]: sends and receives whole messages.
pub trait Transport {
    /// Send one message. It must not wait for the other side to read it
    /// (buffer or queue it instead), because both sides send all their
    /// batches before reading the other side's.
    fn send(&mut self, msg: &Msg) -> Result<()>;
    /// The next message from the other side. A closed connection is
    /// [`SyncError::Disconnected`].
    fn recv(&mut self) -> Result<Msg>;
}

/// Run a whole Sync session with the Device on the other end of `transport`.
/// On failure the other side is told why (when it can still be reached),
/// and every batch received before the failure stays applied.
pub fn run(store: &mut Store, transport: &mut impl Transport) -> Result<SyncReport> {
    let (mut session, hello) = Session::start(store)?;
    transport.send(&hello)?;
    while !session.is_finished() {
        let msg = transport.recv()?;
        match session.handle(store, msg) {
            Ok(replies) => {
                for reply in &replies {
                    transport.send(reply)?;
                }
            }
            Err(error) => {
                if let Some(msg) = Msg::for_error(&error) {
                    // Best effort: the session has failed either way.
                    let _ = transport.send(&msg);
                }
                return Err(error);
            }
        }
    }
    Ok(session.into_report())
}

/// The protocol state of one side of a session, with no I/O, so any
/// transport (blocking or async) can drive it. See [`run`] for a driver.
#[derive(Debug)]
pub struct Session {
    hello_received: bool,
    last_batch_received: bool,
    /// The version vector this side sent in its Done.
    done_sent: Option<VersionVector>,
    finished: bool,
    failed: bool,
    report: SyncReport,
}

impl Session {
    /// Begin a session. Send the returned Hello to the other side first.
    pub fn start(store: &Store) -> Result<(Session, Msg)> {
        let hello = Msg::Hello {
            proto: PROTOCOL_VERSION,
            device_id: store.device_id().to_owned(),
            schema: store.schema_version()?,
            vv: store.version_vector()?,
        };
        let session = Session {
            hello_received: false,
            last_batch_received: false,
            done_sent: None,
            finished: false,
            failed: false,
            report: SyncReport::default(),
        };
        Ok((session, hello))
    }

    /// Handle one message from the other side and return the messages to
    /// send back, in order. A received batch is applied to `store` in one
    /// DB transaction. After an error the session is over: send
    /// [`Msg::for_error`] to the other side if it returns one.
    pub fn handle(&mut self, store: &mut Store, msg: Msg) -> Result<Vec<Msg>> {
        if self.failed || self.finished {
            return Err(protocol_error("message after the end of the session"));
        }
        let result = self.step(store, msg);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    /// Both sides have sent and received Done.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// What the session did so far.
    pub fn report(&self) -> &SyncReport {
        &self.report
    }

    pub fn into_report(self) -> SyncReport {
        self.report
    }

    fn step(&mut self, store: &mut Store, msg: Msg) -> Result<Vec<Msg>> {
        match msg {
            Msg::Error { code, msg } => Err(SyncError::Refused { code, msg }.into()),
            Msg::Hello {
                proto,
                device_id,
                schema,
                vv,
            } => {
                if self.hello_received {
                    return Err(protocol_error("a second Hello"));
                }
                if proto != PROTOCOL_VERSION {
                    return Err(SyncError::IncompatibleProtocol {
                        mine: PROTOCOL_VERSION,
                        theirs: proto,
                    }
                    .into());
                }
                let mine = store.schema_version()?;
                if schema > mine {
                    // I couldn't apply its payloads. (An older peer is fine:
                    // missing fields take their defaults.)
                    return Err(SyncError::UpdateApp {
                        mine,
                        theirs: schema,
                    }
                    .into());
                }
                self.hello_received = true;
                self.report.peer_device_id = device_id;
                let batches = missing_for(store, &vv)?;
                self.report.sent = batches
                    .iter()
                    .map(|b| match b {
                        Msg::Ops { ops, .. } => ops.len(),
                        _ => 0,
                    })
                    .sum();
                Ok(batches)
            }
            Msg::Ops { ops, last } => {
                if !self.hello_received || self.last_batch_received {
                    return Err(protocol_error("a batch of changes outside the exchange"));
                }
                let applied = store.apply_remote(&ops)?;
                self.report.received += applied.new_ops;
                self.report.entities.extend(applied.entities);
                if !last {
                    return Ok(Vec::new());
                }
                self.last_batch_received = true;
                let vv = store.version_vector()?;
                self.done_sent = Some(vv.clone());
                Ok(vec![Msg::Done { vv }])
            }
            Msg::Done { vv } => {
                let Some(mine) = &self.done_sent else {
                    return Err(protocol_error("Done before the last batch of changes"));
                };
                self.report.converged = &vv == mine;
                self.finished = true;
                Ok(Vec::new())
            }
        }
    }
}

fn protocol_error(what: &str) -> Error {
    SyncError::Protocol(what.to_owned()).into()
}

/// The ops this Device has and the peer (with version vector `peer`) lacks,
/// ordered by (origin, seq), in batches that each fit in one message. The
/// last batch is marked `last`; it is empty if there is nothing to send.
fn missing_for(store: &Store, peer: &VersionVector) -> Result<Vec<Msg>> {
    let mut batches = Vec::new();
    let mut batch: Vec<Op> = Vec::new();
    let mut batch_bytes = OPS_MESSAGE_OVERHEAD;
    for (origin, &max) in &store.version_vector()? {
        let has = peer.get(origin).copied().unwrap_or(0);
        if max <= has {
            continue;
        }
        for op in store.ops_since(origin, has)? {
            let size = encoded_len(&op)?;
            if !batch.is_empty()
                && (batch.len() == MAX_BATCH_OPS || batch_bytes + size > MAX_MESSAGE_BYTES)
            {
                batches.push(Msg::Ops {
                    ops: std::mem::take(&mut batch),
                    last: false,
                });
                batch_bytes = OPS_MESSAGE_OVERHEAD;
            }
            batch_bytes += size;
            batch.push(op);
        }
    }
    batches.push(Msg::Ops {
        ops: batch,
        last: true,
    });
    Ok(batches)
}

fn encoded_len(op: &Op) -> Result<usize> {
    let mut bytes = Vec::new();
    ciborium::into_writer(op, &mut bytes).map_err(|e| Error::Codec(e.to_string()))?;
    Ok(bytes.len())
}
