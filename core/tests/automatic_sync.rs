//! Automatic Sync (ticket 16, sync-design.md §3.4) over real TCP on
//! localhost: the PC announces itself while Sync is on, and a phone whose
//! last known address of the PC fails finds it by its device id, syncs, and
//! saves the new address. mDNS runs on the loopback interface only, on a test
//! port, so nothing is sent on the real network.

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use budget_core::accounts::{Account, AccountKind};
use budget_core::sync::{Identity, Listener, Mdns, PairingError, SyncNode};
use budget_core::{Error, Store};
use common::synced_rows;

/// Not 5353, and not the port of `tests/discovery.rs`.
const TEST_MDNS_PORT: u16 = 25354;

struct Device {
    store: Arc<Mutex<Store>>,
    node: SyncNode,
}

impl Device {
    /// A Device that announces itself and finds the others with mDNS.
    fn new() -> Device {
        let device = Device::without_discovery();
        device
            .node
            .set_discovery(Arc::new(Mdns::loopback_only(TEST_MDNS_PORT)));
        device
    }

    fn without_discovery() -> Device {
        let store = Arc::new(Mutex::new(Store::open_in_memory().unwrap()));
        let node = SyncNode::new(store.clone(), Identity::generate().unwrap()).unwrap();
        Device { store, node }
    }

    fn listen(&self) -> Listener {
        self.node.listen("127.0.0.1:0".parse().unwrap()).unwrap()
    }

    fn device_id(&self) -> String {
        self.store.lock().unwrap().device_id().to_owned()
    }

    fn pair_with(&self, pc: &Device) {
        let code = pc
            .node
            .start_pairing(vec!["127.0.0.1".into()])
            .unwrap()
            .to_text();
        self.node.join(&code).unwrap();
    }

    fn add_account(&self, id: &str, name: &str) {
        let account = Account {
            id: id.into(),
            name: name.into(),
            kind: AccountKind::Current,
            opening_minor: 10_000,
            opening_date: "2026-01-01".into(),
            is_main: false,
        };
        self.store
            .lock()
            .unwrap()
            .write(|w| w.upsert(&account))
            .unwrap();
    }

    fn rows(&self) -> Vec<String> {
        synced_rows(&self.store.lock().unwrap())
    }

    fn pc_addr(&self) -> Option<String> {
        self.node.peers().unwrap()[0].last_addr.clone()
    }
}

fn pairing_error(result: budget_core::Result<impl std::fmt::Debug>) -> PairingError {
    match result {
        Err(Error::Pairing(e)) => e,
        other => panic!("expected a pairing error, got {other:?}"),
    }
}

#[test]
fn when_the_pcs_address_changed_the_phone_finds_it_on_the_network_syncs_and_saves_the_new_address()
{
    let pc = Device::new();
    let phone = Device::new();
    let first = pc.listen();
    phone.pair_with(&pc);
    // The PC comes back at another address (here: another port).
    first.stop();
    let second = pc.listen();
    phone.add_account("acc-phone", "Card");

    let report = phone.node.sync_automatically(&pc.device_id()).unwrap();

    assert_eq!(report.sent, 1);
    assert_eq!(pc.rows(), phone.rows());
    assert_eq!(
        phone.pc_addr(),
        Some(format!("127.0.0.1:{}", second.port()))
    );
}

#[test]
fn the_automatic_sync_gives_up_quickly_when_sync_is_off_on_the_pc() {
    let pc = Device::new();
    let phone = Device::new();
    let listener = pc.listen();
    phone.pair_with(&pc);
    listener.stop();

    let started = Instant::now();
    let error = pairing_error(phone.node.sync_automatically(&pc.device_id()));

    // 1.5 s to connect to the last known address, 1.5 s to look on the network.
    assert!(
        started.elapsed() < Duration::from_millis(3500),
        "{:?}",
        started.elapsed()
    );
    assert!(
        matches!(error, PairingError::PeerUnreachable { .. }),
        "{error:?}"
    );
}
