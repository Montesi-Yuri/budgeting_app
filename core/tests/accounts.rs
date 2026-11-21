use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::store::OpKind;
use budget_core::{Error, Store};

fn new_account(name: &str, kind: AccountKind, opening_minor: i64) -> NewAccount {
    NewAccount {
        name: name.into(),
        kind,
        opening_minor,
        opening_date: "2026-11-01".into(),
        is_main: false,
    }
}

#[test]
fn a_created_account_is_listed_with_its_opening_balance_as_its_balance() {
    let mut store = Store::open_in_memory().unwrap();

    let created = accounts::create(
        &mut store,
        new_account("Checking", AccountKind::Current, 125_050),
    )
    .unwrap();

    let listed = accounts::list(&store).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].account, created);
    assert_eq!(listed[0].account.name, "Checking");
    assert_eq!(listed[0].account.opening_date, "2026-11-01");
    assert_eq!(listed[0].balance_minor, 125_050);
}

#[test]
fn credit_card_debt_is_a_negative_balance() {
    let mut store = Store::open_in_memory().unwrap();
    accounts::create(
        &mut store,
        new_account("Visa", AccountKind::CreditCard, -35_000),
    )
    .unwrap();
    let listed = accounts::list(&store).unwrap();
    assert_eq!(listed[0].account.kind, AccountKind::CreditCard);
    assert_eq!(listed[0].balance_minor, -35_000);
}

#[test]
fn renaming_an_account_changes_its_name_and_is_recorded_in_the_sync_log() {
    let mut store = Store::open_in_memory().unwrap();
    let created =
        accounts::create(&mut store, new_account("Checking", AccountKind::Current, 0)).unwrap();

    let renamed = accounts::update(
        &mut store,
        Account {
            name: "Everyday".into(),
            ..created.clone()
        },
    )
    .unwrap();

    assert_eq!(renamed.name, "Everyday");
    assert_eq!(accounts::list(&store).unwrap()[0].account.name, "Everyday");
    let ops = store.ops_since(store.device_id(), 0).unwrap();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[1].kind, OpKind::Upsert);
    assert_eq!(ops[1].entity_id, created.id);
    assert_eq!(
        ops[1].decode_payload::<Account>().unwrap().unwrap().name,
        "Everyday"
    );
}

#[test]
fn editing_an_account_that_does_not_exist_fails() {
    let mut store = Store::open_in_memory().unwrap();
    let created =
        accounts::create(&mut store, new_account("Checking", AccountKind::Current, 0)).unwrap();
    accounts::delete(&mut store, &created.id).unwrap();

    let result = accounts::update(&mut store, created);

    assert!(matches!(result, Err(Error::NotFound { .. })));
}

#[test]
fn a_deleted_account_is_no_longer_listed_and_the_delete_is_logged() {
    let mut store = Store::open_in_memory().unwrap();
    let keep =
        accounts::create(&mut store, new_account("Checking", AccountKind::Current, 0)).unwrap();
    let gone = accounts::create(
        &mut store,
        new_account("Old card", AccountKind::CreditCard, 0),
    )
    .unwrap();

    accounts::delete(&mut store, &gone.id).unwrap();

    let listed: Vec<_> = accounts::list(&store)
        .unwrap()
        .into_iter()
        .map(|a| a.account.id)
        .collect();
    assert_eq!(listed, vec![keep.id]);
    let last_op = store
        .ops_since(store.device_id(), 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!((last_op.kind, last_op.entity_id), (OpKind::Delete, gone.id));
}

#[test]
fn marking_an_account_as_main_unmarks_the_previous_main_in_the_same_change() {
    let mut store = Store::open_in_memory().unwrap();
    let first = accounts::create(
        &mut store,
        NewAccount {
            is_main: true,
            ..new_account("Checking", AccountKind::Current, 0)
        },
    )
    .unwrap();
    let second = accounts::create(
        &mut store,
        new_account("Other bank", AccountKind::Current, 0),
    )
    .unwrap();

    accounts::update(
        &mut store,
        Account {
            is_main: true,
            ..second.clone()
        },
    )
    .unwrap();

    let listed = accounts::list(&store).unwrap();
    let main: Vec<_> = listed
        .iter()
        .filter(|a| a.account.is_main)
        .map(|a| a.account.id.clone())
        .collect();
    assert_eq!(
        main,
        vec![second.id.clone()],
        "only one main Account, listed first"
    );
    assert_eq!(listed[0].account.id, second.id);
    let ops = store.ops_since(store.device_id(), 2).unwrap();
    let mut touched: Vec<_> = ops.iter().map(|o| o.entity_id.clone()).collect();
    touched.sort();
    let mut expected = vec![first.id, second.id];
    expected.sort();
    assert_eq!(touched, expected);
}

#[test]
fn an_account_needs_a_name_and_a_valid_opening_date() {
    let mut store = Store::open_in_memory().unwrap();
    for bad in [
        NewAccount {
            name: "   ".into(),
            ..new_account("x", AccountKind::Current, 0)
        },
        NewAccount {
            opening_date: "2026-02-30".into(),
            ..new_account("x", AccountKind::Current, 0)
        },
        NewAccount {
            opening_date: "01/11/2026".into(),
            ..new_account("x", AccountKind::Current, 0)
        },
    ] {
        assert!(matches!(
            accounts::create(&mut store, bad),
            Err(Error::Invalid(_))
        ));
    }
    assert!(accounts::list(&store).unwrap().is_empty());
    assert!(store.ops_since(store.device_id(), 0).unwrap().is_empty());
}

#[test]
fn names_are_saved_trimmed_and_a_leap_day_is_a_valid_opening_date() {
    let mut store = Store::open_in_memory().unwrap();
    let created = accounts::create(
        &mut store,
        NewAccount {
            name: "  Savings ".into(),
            opening_date: "2028-02-29".into(),
            ..new_account("x", AccountKind::Savings, 0)
        },
    )
    .unwrap();
    assert_eq!(created.name, "Savings");
}
