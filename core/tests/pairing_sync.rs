//! Pairing and Sync between two Devices over real TCP on localhost, encrypted
//! with Noise (sync-design.md §2 and §3.1). Each Device is a `SyncNode` with
//! its own Store; the listener is bound to 127.0.0.1 on a free port.

mod common;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use budget_core::accounts::{Account, AccountKind};
use budget_core::sync::{
    Identity, Listener, PairingError, Peer, SyncEvents, SyncNode, SyncProgress, SyncReport,
};
use budget_core::{Error, Store};
use common::synced_rows;

fn account(id: &str, name: &str) -> Account {
    Account {
        id: id.into(),
        name: name.into(),
        kind: AccountKind::Current,
        opening_minor: 10_000,
        opening_date: "2026-01-01".into(),
        is_main: false,
    }
}

const START: u64 = 1_800_000_000;

/// One Device: its Store and its SyncNode, with a clock the test moves.
struct Device {
    store: Arc<Mutex<Store>>,
    node: SyncNode,
    now: Arc<AtomicU64>,
}

impl Device {
    fn new() -> Device {
        Device::with_events(Box::new(NoEvents))
    }

    fn with_events(events: Box<dyn SyncEvents>) -> Device {
        let store = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
        let now = Arc::new(AtomicU64::new(START));
        let clock = now.clone();
        let node = SyncNode::with_options(
            store.clone(),
            Identity::generate().unwrap(),
            events,
            Box::new(move || clock.load(Ordering::SeqCst)),
        )
        .unwrap();
        Device { store, node, now }
    }

    fn advance(&self, secs: u64) {
        self.now.fetch_add(secs, Ordering::SeqCst);
    }

    fn add_account(&self, id: &str, name: &str) {
        let mut store = self.store.lock().unwrap();
        store.write(|w| w.upsert(&account(id, name))).unwrap();
    }

    fn device_id(&self) -> String {
        self.store.lock().unwrap().device_id().to_owned()
    }

    fn rows(&self) -> Vec<String> {
        synced_rows(&self.store.lock().unwrap())
    }

    fn listen(&self) -> Listener {
        self.node.listen("127.0.0.1:0".parse().unwrap()).unwrap()
    }

    /// "Pair new device": the payload text to paste on the other Device.
    fn pairing_code(&self) -> String {
        self.node
            .start_pairing(vec!["127.0.0.1".into()])
            .unwrap()
            .to_text()
    }
}

struct NoEvents;
impl SyncEvents for NoEvents {}

fn pairing_error(result: budget_core::Result<impl std::fmt::Debug>) -> PairingError {
    match result {
        Err(Error::Pairing(e)) => e,
        other => panic!("expected a pairing error, got {other:?}"),
    }
}

#[test]
fn pasting_the_pairing_code_pairs_both_devices_and_runs_the_first_sync() {
    let pc = Device::new();
    let laptop = Device::new();
    pc.add_account("acc-pc", "Checking");
    laptop.add_account("acc-laptop", "Card");
    let _listener = pc.listen();

    let joined = laptop.node.join(&pc.pairing_code()).unwrap();

    assert_eq!(joined.peer.device_id, pc.device_id());
    assert_eq!((joined.report.sent, joined.report.received), (1, 1));
    let on_laptop = laptop.node.peers().unwrap();
    let on_pc = pc.node.peers().unwrap();
    assert_eq!(on_laptop.len(), 1);
    assert_eq!(on_laptop[0].device_id, pc.device_id());
    assert!(on_laptop[0].last_addr.is_some());
    assert_eq!(on_pc.len(), 1);
    assert_eq!(on_pc[0].device_id, laptop.device_id());
    assert_eq!(pc.rows(), laptop.rows());
}

#[test]
fn a_pairing_code_is_refused_after_five_minutes() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    let code = pc.pairing_code();

    pc.advance(5 * 60);

    assert_eq!(
        pairing_error(laptop.node.join(&code)),
        PairingError::Expired
    );
    assert!(pc.node.peers().unwrap().is_empty());
    assert!(laptop.node.peers().unwrap().is_empty());
}

#[test]
fn a_pairing_code_still_works_just_before_it_expires() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    let code = pc.pairing_code();

    pc.advance(5 * 60 - 1);

    laptop.node.join(&code).unwrap();
}

#[test]
fn a_pairing_code_can_be_used_only_once() {
    let pc = Device::new();
    let laptop = Device::new();
    let intruder = Device::new();
    let _listener = pc.listen();
    let code = pc.pairing_code();
    laptop.node.join(&code).unwrap();

    let reused = intruder.node.join(&code);

    // No pairing is open any more, so the unknown key is cut off during
    // the handshake.
    assert_eq!(pairing_error(reused), PairingError::NotAccepted);
    assert_eq!(pc.node.peers().unwrap().len(), 1);
}

#[test]
fn a_wrong_token_is_refused() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    let mut payload = pc.node.start_pairing(vec!["127.0.0.1".into()]).unwrap();
    payload.token[0] ^= 1;

    let result = laptop.node.join(&payload.to_text());

    assert_eq!(pairing_error(result), PairingError::InvalidToken);
    assert!(pc.node.peers().unwrap().is_empty());
    assert!(laptop.node.peers().unwrap().is_empty());
}

#[test]
fn a_new_pairing_code_replaces_the_previous_one() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    let old = pc.pairing_code();
    let _new = pc.pairing_code();

    assert_eq!(
        pairing_error(laptop.node.join(&old)),
        PairingError::InvalidToken
    );
}

#[test]
fn pairing_needs_sync_to_be_on() {
    let pc = Device::new();

    let result = pc.node.start_pairing(vec!["127.0.0.1".into()]);

    assert_eq!(pairing_error(result), PairingError::NotListening);
}

/// Pair `joiner` with `listener_device`, which must be listening.
fn pair(listener_device: &Device, joiner: &Device) {
    joiner.node.join(&listener_device.pairing_code()).unwrap();
}

#[test]
fn sync_now_sends_and_receives_the_changes_made_since_pairing() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    pair(&pc, &laptop);
    pc.add_account("acc-1", "Checking");
    laptop.add_account("acc-2", "Savings");
    laptop.add_account("acc-3", "Card");

    let report = laptop.node.sync_with(&pc.device_id()).unwrap();

    assert_eq!((report.sent, report.received), (2, 1));
    assert!(report.entities.contains("account"));
    assert!(report.converged);
    assert_eq!(pc.rows(), laptop.rows());
    assert!(laptop.node.peers().unwrap()[0].last_sync_at.is_some());
    assert!(pc.node.peers().unwrap()[0].last_sync_at.is_some());
}

#[test]
fn many_batches_in_both_directions_get_through_without_blocking() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    pair(&pc, &laptop);
    for (device, prefix) in [(&pc, "pc"), (&laptop, "laptop")] {
        let mut store = device.store.lock().unwrap();
        store
            .write(|w| {
                for i in 0..1500 {
                    let name = format!("Account {i} with a fairly long name to fill the batches");
                    w.upsert(&account(&format!("{prefix}-{i}"), &name))?;
                }
                Ok(())
            })
            .unwrap();
    }

    let report = laptop.node.sync_with(&pc.device_id()).unwrap();

    assert_eq!((report.sent, report.received), (1500, 1500));
    assert_eq!(pc.rows(), laptop.rows());
}

#[test]
fn after_unpairing_the_other_device_is_refused_and_nothing_is_exchanged() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    pair(&pc, &laptop);
    pc.node.unpair(&laptop.device_id()).unwrap();
    laptop.add_account("acc-1", "Checking");
    pc.add_account("acc-2", "Savings");

    let result = laptop.node.sync_with(&pc.device_id());

    assert_eq!(pairing_error(result), PairingError::NotAccepted);
    assert!(pc.node.peers().unwrap().is_empty());
    assert_eq!(pc.rows().len(), 1);
    assert_eq!(laptop.rows().len(), 1);
}

#[test]
fn unpairing_on_the_joining_side_forgets_the_listener() {
    let pc = Device::new();
    let laptop = Device::new();
    let _listener = pc.listen();
    pair(&pc, &laptop);

    laptop.node.unpair(&pc.device_id()).unwrap();

    assert!(laptop.node.peers().unwrap().is_empty());
    assert_eq!(
        pairing_error(laptop.node.sync_with(&pc.device_id())),
        PairingError::UnknownPeer(pc.device_id())
    );
}

/// The listener's Noise public key, as the pairing payload shows it.
fn public_key(device: &Device) -> [u8; 32] {
    let payload = device.node.start_pairing(vec!["127.0.0.1".into()]).unwrap();
    device.node.cancel_pairing();
    payload.public_key
}

#[test]
fn a_connection_from_an_unpaired_key_is_closed_before_anything_is_sent_back() {
    use std::io::{Read, Write};
    let pc = Device::new();
    let listener = pc.listen();
    let pc_key = public_key(&pc);
    // A stranger that knows the listener's key and speaks Noise correctly.
    let stranger = snow::Builder::new("Noise_IK_25519_ChaChaPoly_BLAKE2s".parse().unwrap());
    let keys = stranger.generate_keypair().unwrap();
    let mut handshake = snow::Builder::new("Noise_IK_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
        .local_private_key(&keys.private)
        .unwrap()
        .remote_public_key(&pc_key)
        .unwrap()
        .build_initiator()
        .unwrap();
    let mut msg = vec![0u8; 1024];
    let n = handshake.write_message(&[], &mut msg).unwrap();
    let mut tcp = std::net::TcpStream::connect(listener.local_addr()).unwrap();
    tcp.write_all(&(n as u16).to_be_bytes()).unwrap();
    tcp.write_all(&msg[..n]).unwrap();

    let mut received = Vec::new();
    let _ = tcp.read_to_end(&mut received);

    assert!(received.is_empty(), "got {} bytes", received.len());
    assert!(pc.node.peers().unwrap().is_empty());
}

#[test]
fn when_sync_is_turned_off_the_listener_no_longer_accepts_connections() {
    let pc = Device::new();
    let laptop = Device::new();
    let listener = pc.listen();
    pair(&pc, &laptop);

    listener.stop();

    assert!(matches!(
        pairing_error(laptop.node.sync_with(&pc.device_id())),
        PairingError::Unreachable(_)
    ));
    assert_eq!(
        pairing_error(pc.node.start_pairing(vec!["127.0.0.1".into()])),
        PairingError::NotListening
    );
}

/// Records what a SyncNode reports, as the UI would receive it.
#[derive(Default, Clone)]
struct Recorder(Arc<Mutex<Vec<String>>>);

impl Recorder {
    fn log(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl SyncEvents for Recorder {
    fn progress(&self, p: &SyncProgress) {
        self.0
            .lock()
            .unwrap()
            .push(format!("progress {:?}", p.phase));
    }
    fn paired(&self, peer: &Peer) {
        self.0
            .lock()
            .unwrap()
            .push(format!("paired {}", peer.device_id));
    }
    fn synced(&self, report: &SyncReport) {
        let entities: Vec<&str> = report.entities.iter().map(String::as_str).collect();
        self.0.lock().unwrap().push(format!(
            "synced received={} {}",
            report.received,
            entities.join(",")
        ));
    }
}

#[test]
fn the_listener_reports_pairing_and_the_changes_it_received_so_the_ui_can_refresh() {
    let events = Recorder::default();
    let pc = Device::with_events(Box::new(events.clone()));
    let laptop = Device::new();
    let _listener = pc.listen();
    pair(&pc, &laptop);
    laptop.add_account("acc-1", "Checking");

    laptop.node.sync_with(&pc.device_id()).unwrap();

    // The listener's side of the Sync may still be finishing.
    let deadline = Instant::now() + Duration::from_secs(5);
    while events
        .log()
        .iter()
        .filter(|e| e.starts_with("synced"))
        .count()
        < 2
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = events.log();
    assert!(
        log.contains(&format!("paired {}", laptop.device_id())),
        "{log:?}"
    );
    assert!(
        log.contains(&"synced received=1 account".to_owned()),
        "{log:?}"
    );
    assert!(log.contains(&"progress Finished".to_owned()), "{log:?}");
}

#[test]
fn two_instances_with_their_own_data_folders_and_ports_pair_and_converge() {
    let dirs = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let open = |dir: &std::path::Path| {
        let store = Arc::new(Mutex::new(Store::open(dir.join("budget.sqlite3")).unwrap()));
        let identity = Identity::load_or_create(dir.join("sync.key")).unwrap();
        let node = SyncNode::new(store.clone(), identity).unwrap();
        (store, node)
    };
    let write = |store: &Arc<Mutex<Store>>, id: &str| {
        let mut store = store.lock().unwrap();
        store.write(|w| w.upsert(&account(id, id))).unwrap();
    };
    let (store_a, node_a) = open(dirs[0].path());
    let (store_b, node_b) = open(dirs[1].path());
    let listen_a = node_a.listen("127.0.0.1:0".parse().unwrap()).unwrap();
    let listen_b = node_b.listen("127.0.0.1:0".parse().unwrap()).unwrap();
    assert_ne!(listen_a.port(), listen_b.port());
    write(&store_a, "a1");
    write(&store_b, "b1");

    let code = node_a.start_pairing(vec!["127.0.0.1".into()]).unwrap();
    node_b.join(&code.to_text()).unwrap();
    write(&store_a, "a2");
    let a_id = store_a.lock().unwrap().device_id().to_owned();
    assert_eq!(node_b.sync_with(&a_id).unwrap().received, 1);

    assert_eq!(
        synced_rows(&store_a.lock().unwrap()),
        synced_rows(&store_b.lock().unwrap())
    );

    // Restarted, A keeps its key and its paired Device, so B can still sync.
    let port = listen_a.port();
    drop((listen_a, node_a, store_a));
    let (store_a, node_a) = open(dirs[0].path());
    let _listen_a = node_a
        .listen(format!("127.0.0.1:{port}").parse().unwrap())
        .unwrap();
    assert_eq!(node_a.peers().unwrap().len(), 1);
    write(&store_a, "a3");
    assert_eq!(node_b.sync_with(&a_id).unwrap().received, 1);
    assert_eq!(
        synced_rows(&store_a.lock().unwrap()),
        synced_rows(&store_b.lock().unwrap())
    );
}

#[test]
fn pairing_again_with_a_new_code_keeps_one_pairing_and_refreshes_the_address() {
    let pc = Device::new();
    let laptop = Device::new();
    let first = pc.listen();
    pair(&pc, &laptop);
    first.stop();
    let second = pc.listen();
    assert_ne!(second.port(), 0);

    pair(&pc, &laptop);

    assert_eq!(pc.node.peers().unwrap().len(), 1);
    let on_laptop = laptop.node.peers().unwrap();
    assert_eq!(on_laptop.len(), 1);
    assert_eq!(
        on_laptop[0].last_addr,
        Some(format!("127.0.0.1:{}", second.port()))
    );
    laptop.node.sync_with(&pc.device_id()).unwrap();
}
