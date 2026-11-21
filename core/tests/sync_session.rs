//! A Sync session between two Devices over an in-memory channel
//! (sync-design.md §3.2): Hello, batches of changes both ways, Done.

mod common;

use budget_core::accounts::{Account, AccountKind};
use budget_core::sync::{
    self, memory, ErrCode, Msg, Session, SyncError, SyncReport, Transport, PROTOCOL_VERSION,
};
use budget_core::{Error, Result, Store, SCHEMA_VERSION};
use common::{all_ops, sync_pair, synced_rows};

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

#[test]
fn after_a_session_both_devices_have_each_others_changes() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    pc.write(|w| w.upsert(&account("acc-2", "Savings")))
        .unwrap();
    phone
        .write(|w| w.upsert(&account("acc-3", "Card")))
        .unwrap();

    let (on_pc, on_phone) = sync_pair(&mut pc, &mut phone);

    let on_pc = on_pc.unwrap();
    let on_phone = on_phone.unwrap();
    assert_eq!((on_pc.sent, on_pc.received), (2, 1));
    assert_eq!((on_phone.sent, on_phone.received), (1, 2));
    assert_eq!(on_pc.peer_device_id, phone.device_id());
    assert!(on_pc.entities.contains("account"));
    assert!(on_pc.converged && on_phone.converged);
    assert_eq!(synced_rows(&pc), synced_rows(&phone));
    assert_eq!(all_ops(&pc), all_ops(&phone));
}

#[test]
fn two_devices_already_in_sync_exchange_nothing() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    let (first, _) = sync_pair(&mut pc, &mut phone);
    first.unwrap();

    let (on_pc, on_phone) = sync_pair(&mut pc, &mut phone);

    let (on_pc, on_phone) = (on_pc.unwrap(), on_phone.unwrap());
    assert_eq!((on_pc.sent, on_pc.received), (0, 0));
    assert_eq!((on_phone.sent, on_phone.received), (0, 0));
    assert!(on_pc.converged && on_phone.converged);
}

/// A Hello as `store` would send it, but with the given versions.
fn hello_from(store: &Store, proto: u16, schema: u32) -> Msg {
    Msg::Hello {
        proto,
        device_id: store.device_id().to_owned(),
        schema,
        vv: store.version_vector().unwrap(),
    }
}

/// `phone` runs a session against a hand-driven peer that sends `hello`
/// followed by all of `pc`'s changes. Returns the phone's result and every
/// message the peer received.
fn sync_with_peer_sending(
    phone: &mut Store,
    pc: &Store,
    hello: Msg,
) -> (Result<SyncReport>, Vec<Msg>) {
    let (mut phone_end, mut peer) = memory::pair();
    peer.send(&hello).unwrap();
    peer.send(&Msg::Ops {
        ops: all_ops(pc),
        last: true,
    })
    .unwrap();
    // Without a Done from the peer, the phone would wait: close the peer's
    // sending side by keeping only what it receives.
    let result = std::thread::scope(|s| {
        let phone_side = s.spawn(move || sync::run(phone, &mut phone_end));
        let mut received = Vec::new();
        while let Ok(msg) = peer.recv() {
            let is_done = matches!(msg, Msg::Done { .. });
            received.push(msg);
            if is_done {
                break;
            }
        }
        drop(peer);
        (phone_side.join().unwrap(), received)
    });
    result
}

#[test]
fn a_different_protocol_version_is_a_clear_error_and_changes_nothing() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    let hello = hello_from(&pc, PROTOCOL_VERSION + 1, SCHEMA_VERSION);

    let (result, peer_got) = sync_with_peer_sending(&mut phone, &pc, hello);

    let error = result.unwrap_err();
    assert!(
        matches!(error, Error::Sync(SyncError::IncompatibleProtocol { .. })),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("sync protocol version"),
        "{error}"
    );
    assert!(all_ops(&phone).is_empty());
    assert!(synced_rows(&phone).is_empty());
    assert!(peer_got.iter().any(|m| matches!(
        m,
        Msg::Error {
            code: ErrCode::IncompatibleProtocol,
            ..
        }
    )));
}

#[test]
fn a_peer_with_a_newer_schema_is_refused_with_update_app_and_changes_nothing() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    let hello = hello_from(&pc, PROTOCOL_VERSION, SCHEMA_VERSION + 1);

    let (result, peer_got) = sync_with_peer_sending(&mut phone, &pc, hello);

    let error = result.unwrap_err();
    assert!(
        matches!(error, Error::Sync(SyncError::UpdateApp { .. })),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("update the app on this Device"),
        "{error}"
    );
    assert!(all_ops(&phone).is_empty());
    assert!(peer_got.iter().any(|m| matches!(
        m,
        Msg::Error {
            code: ErrCode::UpdateApp,
            ..
        }
    )));
}

#[test]
fn the_newer_device_is_told_that_the_older_one_must_update() {
    let mut phone = Store::open_in_memory().unwrap();
    let (mut phone_end, mut peer) = memory::pair();
    peer.send(&Msg::Error {
        code: ErrCode::UpdateApp,
        msg: "whatever the older app says".into(),
    })
    .unwrap();

    let error = sync::run(&mut phone, &mut phone_end).unwrap_err();

    assert!(
        matches!(
            error,
            Error::Sync(SyncError::Refused {
                code: ErrCode::UpdateApp,
                ..
            })
        ),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("update the app on the other Device"),
        "{error}"
    );
}

#[test]
fn a_peer_with_an_older_schema_is_accepted() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    pc.write(|w| w.upsert(&account("acc-1", "Checking")))
        .unwrap();
    let hello = hello_from(&pc, PROTOCOL_VERSION, SCHEMA_VERSION - 1);

    let (_result, peer_got) = sync_with_peer_sending(&mut phone, &pc, hello);

    assert_eq!(all_ops(&phone), all_ops(&pc));
    assert!(peer_got.iter().any(|m| matches!(m, Msg::Done { .. })));
}

/// A transport that loses the connection after receiving `left` messages.
struct CutOff<T> {
    inner: T,
    left: usize,
}

impl<T: Transport> Transport for CutOff<T> {
    fn send(&mut self, msg: &Msg) -> Result<()> {
        self.inner.send(msg)
    }

    fn recv(&mut self) -> Result<Msg> {
        if self.left == 0 {
            return Err(SyncError::Disconnected.into());
        }
        self.left -= 1;
        self.inner.recv()
    }
}

#[test]
fn a_session_cut_off_halfway_is_finished_by_the_next_one_without_duplicates() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    // Enough changes for several batches.
    for i in 0..500 {
        pc.write(|w| w.upsert(&account(&format!("acc-{i:03}"), "Checking")))
            .unwrap();
    }
    phone
        .write(|w| w.upsert(&account("phone-1", "Card")))
        .unwrap();

    // The phone receives the PC's Hello and one batch, then the Wi-Fi drops.
    let (mut end_pc, end_phone) = memory::pair();
    let mut end_phone = CutOff {
        inner: end_phone,
        left: 2,
    };
    let (on_pc, on_phone) = std::thread::scope(|s| {
        let phone = &mut phone;
        let on_phone = s.spawn(move || {
            let result = sync::run(phone, &mut end_phone);
            drop(end_phone);
            result
        });
        let on_pc = sync::run(&mut pc, &mut end_pc);
        (on_pc, on_phone.join().unwrap())
    });
    assert!(
        matches!(on_phone, Err(Error::Sync(SyncError::Disconnected))),
        "{on_phone:?}"
    );
    assert!(on_pc.is_err());
    let kept = all_ops(&phone).len();
    assert!(kept > 1 && kept < 501, "the phone kept {kept} changes");

    let (on_pc, on_phone) = sync_pair(&mut pc, &mut phone);

    assert_eq!(on_phone.unwrap().received, 501 - kept);
    assert!(on_pc.unwrap().converged);
    assert_eq!(all_ops(&phone).len(), 501);
    assert_eq!(all_ops(&phone), all_ops(&pc));
    assert_eq!(synced_rows(&phone), synced_rows(&pc));
}

#[test]
fn with_three_devices_a_change_reaches_c_through_b() {
    let mut a = Store::open_in_memory().unwrap();
    let mut b = Store::open_in_memory().unwrap();
    let mut c = Store::open_in_memory().unwrap();
    a.write(|w| w.upsert(&account("acc-a", "Made on A")))
        .unwrap();
    c.write(|w| w.upsert(&account("acc-c", "Made on C")))
        .unwrap();

    let (ab, _) = sync_pair(&mut a, &mut b);
    ab.unwrap();
    let (bc, cb) = sync_pair(&mut b, &mut c);

    assert_eq!(bc.unwrap().received, 1);
    assert_eq!(cb.unwrap().received, 1);
    assert_eq!(synced_rows(&c), synced_rows(&b));
    assert!(synced_rows(&c).iter().any(|row| row.contains("Made on A")));
    assert_eq!(c.version_vector().unwrap().get(a.device_id()), Some(&1));
}

#[test]
fn every_message_fits_in_one_noise_frame_even_with_large_rows() {
    let mut pc = Store::open_in_memory().unwrap();
    let mut phone = Store::open_in_memory().unwrap();
    let long_name = "x".repeat(3_000);
    for i in 0..100 {
        pc.write(|w| w.upsert(&account(&format!("acc-{i:03}"), &long_name)))
            .unwrap();
    }
    let (mut session, _hello) = Session::start(&pc).unwrap();

    let replies = session
        .handle(
            &mut pc,
            hello_from(&phone, PROTOCOL_VERSION, SCHEMA_VERSION),
        )
        .unwrap();

    assert!(
        replies.len() > 1,
        "the changes should be split into batches"
    );
    for reply in &replies {
        assert!(sync::encode(reply).unwrap().len() <= sync::protocol::MAX_MESSAGE_BYTES);
        let Msg::Ops { ops, .. } = sync::decode(&sync::encode(reply).unwrap()).unwrap() else {
            panic!("expected a batch, got {reply:?}");
        };
        phone.apply_remote(&ops).unwrap();
    }
    assert_eq!(all_ops(&phone).len(), 100);
}
