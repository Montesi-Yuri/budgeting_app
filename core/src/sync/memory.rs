//! An in-process [`Transport`]: two connected ends, each message encoded to
//! CBOR bytes as it would be on the network. For tests, and to sync two
//! Stores in one process.

use std::sync::mpsc::{channel, Receiver, Sender};

use crate::error::Result;
use crate::sync::protocol::{decode, encode, Msg, SyncError};
use crate::sync::session::Transport;

/// One end of an in-memory connection. Dropping it closes the connection.
#[derive(Debug)]
pub struct MemoryTransport {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

/// Two connected ends: what one sends, the other receives.
pub fn pair() -> (MemoryTransport, MemoryTransport) {
    let (a_tx, b_rx) = channel();
    let (b_tx, a_rx) = channel();
    (
        MemoryTransport { tx: a_tx, rx: a_rx },
        MemoryTransport { tx: b_tx, rx: b_rx },
    )
}

impl Transport for MemoryTransport {
    fn send(&mut self, msg: &Msg) -> Result<()> {
        self.tx
            .send(encode(msg)?)
            .map_err(|_| SyncError::Disconnected.into())
    }

    fn recv(&mut self) -> Result<Msg> {
        let bytes = self.rx.recv().map_err(|_| SyncError::Disconnected)?;
        decode(&bytes)
    }
}
