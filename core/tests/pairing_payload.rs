//! The pairing payload (sync-design.md §2.1): what the listener shows and
//! the other Device pastes (later: scans as a QR code).

use budget_core::sync::pairing::PairingPayload;
use budget_core::sync::PairingError;
use budget_core::Error;

fn payload() -> PairingPayload {
    PairingPayload {
        device_id: "8f1c0000-0000-4000-8000-000000000001".into(),
        name: "Yuri-PC".into(),
        addrs: vec!["192.168.1.20".into(), "10.0.0.5".into()],
        port: 47321,
        public_key: [7; 32],
        token: [9; 32],
        expires_at: 1_731_900_300,
    }
}

#[test]
fn the_payload_is_a_bsync_text_that_reads_back_the_same() {
    let text = payload().to_text();

    assert!(text.starts_with("bsync:1:"), "{text}");
    assert!(
        text.len() < 400,
        "small enough for a QR code: {}",
        text.len()
    );
    assert_eq!(PairingPayload::parse(&text).unwrap(), payload());
}

#[test]
fn pasted_text_with_surrounding_whitespace_is_accepted() {
    let text = format!("  {}\n", payload().to_text());

    assert_eq!(PairingPayload::parse(&text).unwrap(), payload());
}

#[test]
fn text_that_is_not_a_pairing_payload_is_rejected() {
    for text in ["", "hello", "bsync:1:", "bsync:1:!!!", "bsync:2:AAAA"] {
        let err = PairingPayload::parse(text).unwrap_err();
        assert!(
            matches!(err, Error::Pairing(PairingError::BadPayload(_))),
            "{text:?}: {err}"
        );
    }
}
