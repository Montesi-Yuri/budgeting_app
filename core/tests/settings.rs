use budget_core::settings;
use budget_core::store::OpKind;
use budget_core::{Error, Store};

#[test]
fn the_currency_is_unset_until_the_user_chooses_one() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(settings::currency(&store).unwrap(), None);
}

#[test]
fn setting_the_currency_is_a_synced_change() {
    let mut store = Store::open_in_memory().unwrap();

    settings::set_currency(&mut store, "eur").unwrap();

    assert_eq!(settings::currency(&store).unwrap().as_deref(), Some("EUR"));
    let ops = store.ops_since(store.device_id(), 0).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(
        (
            ops[0].entity.as_str(),
            ops[0].entity_id.as_str(),
            ops[0].kind
        ),
        ("setting", "currency", OpKind::Upsert)
    );
}

#[test]
fn changing_the_currency_replaces_it() {
    let mut store = Store::open_in_memory().unwrap();
    settings::set_currency(&mut store, "EUR").unwrap();
    settings::set_currency(&mut store, "GBP").unwrap();
    assert_eq!(settings::currency(&store).unwrap().as_deref(), Some("GBP"));
}

#[test]
fn a_currency_must_be_a_three_letter_iso_code() {
    let mut store = Store::open_in_memory().unwrap();
    for bad in ["", "EU", "EURO", "€", "E1R"] {
        assert!(
            matches!(
                settings::set_currency(&mut store, bad),
                Err(Error::Invalid(_))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(settings::currency(&store).unwrap(), None);
}

#[test]
fn the_savings_target_is_zero_until_the_user_sets_one() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(settings::savings_target_minor(&store).unwrap(), 0);
}

#[test]
fn setting_the_savings_target_is_a_synced_change() {
    let mut store = Store::open_in_memory().unwrap();

    settings::set_savings_target(&mut store, 30_000).unwrap();

    assert_eq!(settings::savings_target_minor(&store).unwrap(), 30_000);
    let ops = store.ops_since(store.device_id(), 0).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(
        (ops[0].entity.as_str(), ops[0].entity_id.as_str()),
        ("setting", "savings_target_minor")
    );
}

#[test]
fn a_savings_target_cannot_be_negative() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(matches!(
        settings::set_savings_target(&mut store, -1),
        Err(Error::Invalid(_))
    ));
    assert_eq!(settings::savings_target_minor(&store).unwrap(), 0);
}
