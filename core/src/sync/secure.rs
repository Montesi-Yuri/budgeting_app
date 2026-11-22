//! The encrypted connection (sync-design.md §2.2, §3.1): a Noise IK
//! handshake over TCP, then one Noise transport message per [`Msg`].
//!
//! Every frame on the wire, handshake messages included, is a `u16`
//! big-endian length followed by that many bytes.
//!
//! [`SecureTransport`] implements [`Transport`]. Its `send` only queues the
//! message for a writer thread, so it never waits for the other side to
//! read (both sides send all their batches before reading).

use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use snow::StatelessTransportState;

use crate::error::{Error, Result};
use crate::sync::identity::{noise_error, Identity};
use crate::sync::pairing::PairingError;
use crate::sync::protocol::{decode, encode, Msg, SyncError};
use crate::sync::session::Transport;

/// The Noise protocol: the initiator knows the responder's static key in
/// advance (from the pairing payload) and sends its own, encrypted.
pub(crate) const NOISE_PARAMS: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

/// Noise's limit for one message, and its authentication tag.
const MAX_FRAME: usize = 65_535;
const TAG_LEN: usize = 16;

/// How long a read or write may wait for the other Device.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// An encrypted connection to another Device.
pub(crate) struct SecureTransport {
    reader: TcpStream,
    state: Arc<StatelessTransportState>,
    recv_nonce: u64,
    outbox: Option<Sender<Vec<u8>>>,
    writer: Option<JoinHandle<()>>,
    remote_key: [u8; 32],
}

impl SecureTransport {
    /// The other Device's Noise static key, proven by the handshake.
    pub fn remote_key(&self) -> &[u8; 32] {
        &self.remote_key
    }

    /// Send everything queued, then close the connection.
    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.outbox.take();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        let _ = self.reader.shutdown(Shutdown::Both);
    }
}

impl Drop for SecureTransport {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Transport for SecureTransport {
    fn send(&mut self, msg: &Msg) -> Result<()> {
        let bytes = encode(msg)?;
        if bytes.len() + TAG_LEN > MAX_FRAME {
            return Err(Error::Codec(format!(
                "a sync message of {} bytes is too big for one frame",
                bytes.len()
            )));
        }
        self.outbox
            .as_ref()
            .and_then(|outbox| outbox.send(bytes).ok())
            .ok_or_else(|| SyncError::Disconnected.into())
    }

    fn recv(&mut self) -> Result<Msg> {
        let frame = read_frame(&mut self.reader)?;
        let mut plain = vec![0u8; frame.len()];
        let n = self
            .state
            .read_message(self.recv_nonce, &frame, &mut plain)
            .map_err(noise_error)?;
        self.recv_nonce += 1;
        decode(&plain[..n])
    }
}

/// Connect to `addr` and run the handshake as the initiator, expecting the
/// other Device to hold `remote_key`. A Device that closes the connection
/// during the handshake doesn't know this one: [`PairingError::NotAccepted`].
pub(crate) fn connect(
    addr: SocketAddr,
    timeout: Duration,
    identity: &Identity,
    remote_key: &[u8; 32],
) -> Result<SecureTransport> {
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|_| PairingError::Unreachable(addr.to_string()))?;
    configure(&stream)?;
    let mut handshake = builder()?
        .local_private_key(identity.private_key())
        .and_then(|b| b.remote_public_key(remote_key))
        .and_then(|b| b.build_initiator())
        .map_err(noise_error)?;
    let mut buf = vec![0u8; MAX_FRAME];
    let n = handshake
        .write_message(&[], &mut buf)
        .map_err(noise_error)?;
    write_frame(&mut stream, &buf[..n]).map_err(net_error)?;
    let reply = match read_frame(&mut stream) {
        Ok(reply) => reply,
        Err(Error::Sync(SyncError::Disconnected)) => return Err(PairingError::NotAccepted.into()),
        Err(e) => return Err(e),
    };
    handshake
        .read_message(&reply, &mut buf)
        .map_err(noise_error)?;
    let state = handshake
        .into_stateless_transport_mode()
        .map_err(noise_error)?;
    start(stream, state, *remote_key)
}

/// Run the handshake as the responder on an accepted connection. `admit`
/// sees the initiator's static key as soon as the first handshake message
/// is read; if it says no, the connection is closed before anything is sent
/// back, and the result is `Ok(None)`.
pub(crate) fn accept(
    mut stream: TcpStream,
    identity: &Identity,
    admit: impl FnOnce(&[u8; 32]) -> bool,
) -> Result<Option<SecureTransport>> {
    configure(&stream)?;
    let mut handshake = builder()?
        .local_private_key(identity.private_key())
        .and_then(|b| b.build_responder())
        .map_err(noise_error)?;
    let first = read_frame(&mut stream)?;
    let mut buf = vec![0u8; MAX_FRAME];
    if handshake.read_message(&first, &mut buf).is_err() {
        // Not for this Device's key, or not Noise at all.
        let _ = stream.shutdown(Shutdown::Both);
        return Ok(None);
    }
    let remote_key: [u8; 32] = handshake
        .get_remote_static()
        .and_then(|k| k.try_into().ok())
        .ok_or_else(|| Error::Net("the other Device sent no key".to_owned()))?;
    if !admit(&remote_key) {
        let _ = stream.shutdown(Shutdown::Both);
        return Ok(None);
    }
    let n = handshake
        .write_message(&[], &mut buf)
        .map_err(noise_error)?;
    write_frame(&mut stream, &buf[..n]).map_err(net_error)?;
    let state = handshake
        .into_stateless_transport_mode()
        .map_err(noise_error)?;
    start(stream, state, remote_key).map(Some)
}

fn builder<'a>() -> Result<snow::Builder<'a>> {
    Ok(snow::Builder::new(
        NOISE_PARAMS.parse().map_err(noise_error)?,
    ))
}

fn configure(stream: &TcpStream) -> Result<()> {
    stream.set_nodelay(true).map_err(net_error)?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(net_error)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(net_error)?;
    Ok(())
}

/// Start the writer thread: it encrypts each queued message in order and
/// writes it, until the queue is closed or the connection fails.
fn start(
    stream: TcpStream,
    state: StatelessTransportState,
    remote_key: [u8; 32],
) -> Result<SecureTransport> {
    let mut out = stream.try_clone().map_err(net_error)?;
    let state = Arc::new(state);
    let (outbox, queue) = channel::<Vec<u8>>();
    let writer_state = state.clone();
    let writer = std::thread::spawn(move || {
        let mut buf = vec![0u8; MAX_FRAME];
        for (nonce, plain) in (0u64..).zip(queue) {
            let Ok(n) = writer_state.write_message(nonce, &plain, &mut buf) else {
                break;
            };
            if write_frame(&mut out, &buf[..n]).is_err() {
                break;
            }
        }
        let _ = out.flush();
    });
    Ok(SecureTransport {
        reader: stream,
        state,
        recv_nonce: 0,
        outbox: Some(outbox),
        writer: Some(writer),
        remote_key,
    })
}

fn write_frame(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    let len = u16::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too big"))?;
    let mut frame = Vec::with_capacity(bytes.len() + 2);
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(bytes);
    stream.write_all(&frame)
}

/// One frame. A closed connection is [`SyncError::Disconnected`].
fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len = [0u8; 2];
    read_exact(stream, &mut len)?;
    let mut frame = vec![0u8; u16::from_be_bytes(len) as usize];
    read_exact(stream, &mut frame)?;
    Ok(frame)
}

fn read_exact(stream: &mut TcpStream, buf: &mut [u8]) -> Result<()> {
    stream.read_exact(buf).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::BrokenPipe => SyncError::Disconnected.into(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
            Error::Net("the other Device stopped responding".to_owned())
        }
        _ => net_error(e),
    })
}

fn net_error(e: io::Error) -> Error {
    Error::Net(e.to_string())
}
