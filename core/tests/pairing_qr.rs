//! The pairing payload as a QR code (ticket 15): the PC shows it, the phone
//! scans it. Decoded here with an independent QR reader (`rqrr`).

use budget_core::sync::pairing::{PairingPayload, Qr};

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

/// Read a QR code the way a camera would: as a greyscale picture with a
/// white quiet zone, 4 pixels per module.
fn scan(qr: &Qr) -> String {
    const SCALE: usize = 4;
    const QUIET: usize = 4;
    let side = (qr.size() + 2 * QUIET) * SCALE;
    let mut image = rqrr::PreparedImage::prepare_from_greyscale(side, side, |x, y| {
        let (mx, my) = (x / SCALE, y / SCALE);
        let inside =
            (QUIET..QUIET + qr.size()).contains(&mx) && (QUIET..QUIET + qr.size()).contains(&my);
        if inside && qr.is_dark(mx - QUIET, my - QUIET) {
            0
        } else {
            255
        }
    });
    let grids = image.detect_grids();
    assert_eq!(grids.len(), 1, "one QR code in the picture");
    grids[0].decode().expect("the QR code decodes").1
}

#[test]
fn the_qr_code_reads_back_as_the_pairing_code() {
    let qr = payload().to_qr();

    let scanned = scan(&qr);

    assert_eq!(scanned, payload().to_text());
    assert_eq!(PairingPayload::parse(&scanned).unwrap(), payload());
}

#[test]
fn the_qr_code_is_an_svg_with_a_quiet_zone_around_it() {
    let qr = payload().to_qr();

    let svg = qr.to_svg();

    assert!(svg.starts_with("<svg"), "{svg}");
    let side = qr.size() + 8;
    assert!(
        svg.contains(&format!("viewBox=\"0 0 {side} {side}\"")),
        "{svg}"
    );
    assert!(svg.contains("fill=\"#fff\""), "white background: {svg}");
    assert!(svg.trim_end().ends_with("</svg>"));
}
