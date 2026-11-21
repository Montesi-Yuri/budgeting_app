//! Manual Transactions on an Account, and Account balances (CONTEXT.md,
//! sync-design §1.7).

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::store::OpKind;
use budget_core::transactions::{
    self, CategorySource, NewManualTransaction, TransactionEdit, TxnSource,
};
use budget_core::{Error, Store};

/// An Account with an Opening balance of 1,000.00 on 2026-11-01.
fn checking(store: &mut Store) -> Account {
    accounts::create(
        store,
        NewAccount {
            name: "Checking".into(),
            kind: AccountKind::Current,
            opening_minor: 100_000,
            opening_date: "2026-11-01".into(),
            is_main: true,
        },
    )
    .unwrap()
}

fn manual(
    account: &Account,
    date: &str,
    amount_minor: i64,
    description: &str,
) -> NewManualTransaction {
    NewManualTransaction {
        account_id: account.id.clone(),
        date: date.into(),
        amount_minor,
        description: description.into(),
        category_id: None,
        note: None,
    }
}

fn balance(store: &Store, account: &Account) -> i64 {
    accounts::list(store)
        .unwrap()
        .into_iter()
        .find(|a| a.account.id == account.id)
        .unwrap()
        .balance_minor
}

#[test]
fn the_balance_is_the_opening_balance_plus_the_transactions_after_the_opening_date() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);

    transactions::create_manual(
        &mut store,
        manual(&account, "2026-11-03", -1_250, "Coffee beans"),
    )
    .unwrap();
    transactions::create_manual(
        &mut store,
        manual(&account, "2026-11-27", 250_000, "Salary"),
    )
    .unwrap();
    // On or before the opening date: already part of the Opening balance.
    transactions::create_manual(&mut store, manual(&account, "2026-11-01", -9_999, "Old")).unwrap();
    transactions::create_manual(&mut store, manual(&account, "2026-10-15", -5_000, "Older"))
        .unwrap();

    assert_eq!(balance(&store, &account), 100_000 - 1_250 + 250_000);
}

fn category(
    store: &mut Store,
    name: &str,
    kind: CategoryKind,
    parent: Option<&Category>,
) -> Category {
    categories::create(
        store,
        NewCategory {
            name: name.into(),
            kind,
            parent_id: parent.map(|p| p.id.clone()),
        },
    )
    .unwrap()
}

fn with_category(new: NewManualTransaction, category: &Category) -> NewManualTransaction {
    NewManualTransaction {
        category_id: Some(category.id.clone()),
        ..new
    }
}

#[test]
fn an_accounts_transactions_are_listed_newest_first_with_their_category() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let other = accounts::create(
        &mut store,
        NewAccount {
            name: "Visa".into(),
            kind: AccountKind::CreditCard,
            opening_minor: 0,
            opening_date: "2026-11-01".into(),
            is_main: false,
        },
    )
    .unwrap();
    let food = category(&mut store, "Food", CategoryKind::Expense, None);
    let groceries = category(&mut store, "Groceries", CategoryKind::Expense, Some(&food));

    transactions::create_manual(
        &mut store,
        with_category(manual(&account, "2026-11-03", -4_000, "Market"), &groceries),
    )
    .unwrap();
    transactions::create_manual(&mut store, manual(&account, "2026-11-10", -900, "Kiosk")).unwrap();
    transactions::create_manual(&mut store, manual(&account, "2026-11-05", -1_500, "Bakery"))
        .unwrap();
    transactions::create_manual(&mut store, manual(&account, "2026-11-10", -300, "Coffee"))
        .unwrap();
    transactions::create_manual(
        &mut store,
        manual(&other, "2026-11-04", -2_000, "Elsewhere"),
    )
    .unwrap();

    let listed = transactions::list_for_account(&store, &account.id).unwrap();

    let rows: Vec<_> = listed
        .iter()
        .map(|t| {
            (
                t.transaction.description.as_str(),
                t.category_name.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Coffee", None),
            ("Kiosk", None),
            ("Bakery", None),
            ("Market", Some("Groceries")),
        ],
        "same-day Transactions: the one entered last comes first"
    );
    assert_eq!(listed[3].category_kind, Some(CategoryKind::Expense));
    assert_eq!(listed[3].account_name, "Checking");
}

#[test]
fn a_transaction_whose_category_was_deleted_is_uncategorised() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let gifts = category(&mut store, "Gifts", CategoryKind::Expense, None);
    let txn = transactions::create_manual(
        &mut store,
        with_category(manual(&account, "2026-11-03", -4_000, "Toy"), &gifts),
    )
    .unwrap();

    categories::delete(&mut store, &gifts.id).unwrap();

    let listed = transactions::list_for_account(&store, &account.id).unwrap();
    assert_eq!(listed[0].transaction.id, txn.id);
    assert_eq!(listed[0].category_name, None);
    assert_eq!(listed[0].category_kind, None);
}

fn edit_of(txn: &budget_core::transactions::Transaction) -> TransactionEdit {
    TransactionEdit {
        id: txn.id.clone(),
        account_id: txn.account_id.clone(),
        date: txn.date.clone(),
        amount_minor: txn.amount_minor,
        description: txn.description.clone(),
        category_id: txn.category_id.clone(),
        note: txn.note.clone(),
    }
}

#[test]
fn editing_a_manual_transaction_saves_every_field_and_updates_the_balance() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let food = category(&mut store, "Food", CategoryKind::Expense, None);
    let txn =
        transactions::create_manual(&mut store, manual(&account, "2026-11-03", -1_000, "Lunch"))
            .unwrap();
    assert_eq!(txn.source, TxnSource::Manual);

    let edited = transactions::update(
        &mut store,
        TransactionEdit {
            date: "2026-11-04".into(),
            amount_minor: -1_800,
            description: "Lunch with Ann".into(),
            category_id: Some(food.id.clone()),
            note: Some("my share".into()),
            ..edit_of(&txn)
        },
    )
    .unwrap();

    let listed = transactions::list_for_account(&store, &account.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].transaction, edited);
    assert_eq!(edited.date, "2026-11-04");
    assert_eq!(edited.amount_minor, -1_800);
    assert_eq!(edited.description, "Lunch with Ann");
    assert_eq!(edited.note.as_deref(), Some("my share"));
    assert_eq!(listed[0].category_name.as_deref(), Some("Food"));
    assert_eq!(balance(&store, &account), 100_000 - 1_800);
    let last = store
        .ops_since(store.device_id(), 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!((last.entity.as_str(), last.kind), ("txn", OpKind::Upsert));
    assert_eq!(
        last.decode_payload::<budget_core::transactions::Transaction>()
            .unwrap()
            .unwrap(),
        edited
    );
}

#[test]
fn a_transaction_can_be_moved_to_another_account() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let card = accounts::create(
        &mut store,
        NewAccount {
            name: "Visa".into(),
            kind: AccountKind::CreditCard,
            opening_minor: 0,
            opening_date: "2026-11-01".into(),
            is_main: false,
        },
    )
    .unwrap();
    let txn =
        transactions::create_manual(&mut store, manual(&account, "2026-11-03", -1_000, "Lunch"))
            .unwrap();

    transactions::update(
        &mut store,
        TransactionEdit {
            account_id: card.id.clone(),
            ..edit_of(&txn)
        },
    )
    .unwrap();

    assert_eq!(balance(&store, &account), 100_000);
    assert_eq!(balance(&store, &card), -1_000);
}

#[test]
fn a_deleted_transaction_leaves_the_list_and_the_balance_and_the_delete_is_logged() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let keep =
        transactions::create_manual(&mut store, manual(&account, "2026-11-03", -1_000, "Lunch"))
            .unwrap();
    let gone =
        transactions::create_manual(&mut store, manual(&account, "2026-11-04", -500, "Typo"))
            .unwrap();

    transactions::delete(&mut store, &gone.id).unwrap();

    let listed: Vec<_> = transactions::list_for_account(&store, &account.id)
        .unwrap()
        .into_iter()
        .map(|t| t.transaction.id)
        .collect();
    assert_eq!(listed, [keep.id]);
    assert_eq!(balance(&store, &account), 100_000 - 1_000);
    let last = store
        .ops_since(store.device_id(), 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        (last.kind, last.entity_id),
        (OpKind::Delete, gone.id.clone())
    );
    assert!(matches!(
        transactions::delete(&mut store, &gone.id),
        Err(Error::NotFound { .. })
    ));
    assert!(matches!(
        transactions::update(&mut store, edit_of(&gone)),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn a_transaction_needs_a_live_account_a_valid_date_a_description_a_live_category_and_an_amount() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let closed = accounts::create(
        &mut store,
        NewAccount {
            name: "Closed".into(),
            kind: AccountKind::Savings,
            opening_minor: 0,
            opening_date: "2026-11-01".into(),
            is_main: false,
        },
    )
    .unwrap();
    accounts::delete(&mut store, &closed.id).unwrap();
    let gone = category(&mut store, "Gone", CategoryKind::Expense, None);
    categories::delete(&mut store, &gone.id).unwrap();
    let ops_before = store.ops_since(store.device_id(), 0).unwrap().len();
    let ok = manual(&account, "2026-11-03", -1_000, "Lunch");

    for bad in [
        NewManualTransaction {
            account_id: closed.id.clone(),
            ..ok.clone()
        },
        NewManualTransaction {
            account_id: "nope".into(),
            ..ok.clone()
        },
        NewManualTransaction {
            date: "2026-11-31".into(),
            ..ok.clone()
        },
        NewManualTransaction {
            description: "  ".into(),
            ..ok.clone()
        },
        NewManualTransaction {
            category_id: Some(gone.id.clone()),
            ..ok.clone()
        },
        NewManualTransaction {
            amount_minor: 0,
            ..ok.clone()
        },
    ] {
        let result = transactions::create_manual(&mut store, bad.clone());
        assert!(
            matches!(result, Err(Error::Invalid(_))),
            "{bad:?} gave {result:?}"
        );
    }
    assert_eq!(
        store.ops_since(store.device_id(), 0).unwrap().len(),
        ops_before
    );

    let txn = transactions::create_manual(&mut store, ok).unwrap();
    let result = transactions::update(
        &mut store,
        TransactionEdit {
            date: "03/11/2026".into(),
            ..edit_of(&txn)
        },
    );
    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[test]
fn the_description_and_note_are_saved_trimmed_and_an_empty_note_is_no_note() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let txn = transactions::create_manual(
        &mut store,
        NewManualTransaction {
            note: Some("   ".into()),
            ..manual(&account, "2026-11-03", -1_000, "  Lunch ")
        },
    )
    .unwrap();
    assert_eq!(txn.description, "Lunch");
    assert_eq!(txn.note, None);
}

#[test]
fn editing_a_transaction_whose_category_was_deleted_keeps_working() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let gifts = category(&mut store, "Gifts", CategoryKind::Expense, None);
    let txn = transactions::create_manual(
        &mut store,
        with_category(manual(&account, "2026-11-03", -4_000, "Toy"), &gifts),
    )
    .unwrap();
    categories::delete(&mut store, &gifts.id).unwrap();

    let edited = transactions::update(
        &mut store,
        TransactionEdit {
            note: Some("for Max".into()),
            ..edit_of(&txn)
        },
    )
    .unwrap();

    assert_eq!(edited.note.as_deref(), Some("for Max"));
}

#[test]
fn the_uncategorised_list_shows_every_transaction_without_a_category_newest_first() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let card = accounts::create(
        &mut store,
        NewAccount {
            name: "Visa".into(),
            kind: AccountKind::CreditCard,
            opening_minor: 0,
            opening_date: "2026-11-01".into(),
            is_main: false,
        },
    )
    .unwrap();
    let food = category(&mut store, "Food", CategoryKind::Expense, None);
    let gifts = category(&mut store, "Gifts", CategoryKind::Expense, None);
    transactions::create_manual(
        &mut store,
        with_category(manual(&account, "2026-11-02", -100, "Bread"), &food),
    )
    .unwrap();
    transactions::create_manual(&mut store, manual(&account, "2026-11-03", -200, "Kiosk")).unwrap();
    transactions::create_manual(&mut store, manual(&card, "2026-11-05", -300, "Online shop"))
        .unwrap();
    transactions::create_manual(
        &mut store,
        with_category(manual(&card, "2026-11-04", -400, "Toy"), &gifts),
    )
    .unwrap();
    categories::delete(&mut store, &gifts.id).unwrap();

    let rows: Vec<_> = transactions::list_uncategorised(&store)
        .unwrap()
        .into_iter()
        .map(|t| (t.transaction.description, t.account_name))
        .collect();

    assert_eq!(
        rows,
        [
            ("Online shop".to_owned(), "Visa".to_owned()),
            ("Toy".to_owned(), "Visa".to_owned()),
            ("Kiosk".to_owned(), "Checking".to_owned()),
        ]
    );
}

#[test]
fn assigning_a_category_from_the_uncategorised_list_marks_it_as_set_by_the_user() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let food = category(&mut store, "Food", CategoryKind::Expense, None);
    let txn =
        transactions::create_manual(&mut store, manual(&account, "2026-11-03", -200, "Kiosk"))
            .unwrap();
    assert_eq!(txn.category_source, None);

    let assigned = transactions::set_category(&mut store, &txn.id, Some(&food.id)).unwrap();

    assert_eq!(assigned.category_id.as_deref(), Some(food.id.as_str()));
    assert_eq!(assigned.category_source, Some(CategorySource::User));
    assert!(transactions::list_uncategorised(&store).unwrap().is_empty());
    let last = store
        .ops_since(store.device_id(), 0)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        (last.entity_id, last.kind),
        (txn.id.clone(), OpKind::Upsert)
    );

    let cleared = transactions::set_category(&mut store, &txn.id, None).unwrap();
    assert_eq!((cleared.category_id, cleared.category_source), (None, None));
    assert_eq!(transactions::list_uncategorised(&store).unwrap().len(), 1);
}

#[test]
fn a_category_chosen_by_hand_is_marked_as_set_by_the_user() {
    let mut store = Store::open_in_memory().unwrap();
    let account = checking(&mut store);
    let food = category(&mut store, "Food", CategoryKind::Expense, None);
    let fun = category(&mut store, "Fun", CategoryKind::Expense, None);

    let created = transactions::create_manual(
        &mut store,
        with_category(manual(&account, "2026-11-03", -200, "Kiosk"), &food),
    )
    .unwrap();
    assert_eq!(created.category_source, Some(CategorySource::User));

    // A Transaction categorised by a Rule (ticket 04), written as the import will.
    let by_rule = budget_core::transactions::Transaction {
        id: "imported-1".into(),
        source: TxnSource::Import,
        category_source: Some(CategorySource::Rule),
        ..created.clone()
    };
    store.write(|w| w.upsert(&by_rule)).unwrap();

    let note_only = transactions::update(
        &mut store,
        TransactionEdit {
            note: Some("x".into()),
            ..edit_of(&by_rule)
        },
    )
    .unwrap();
    assert_eq!(
        note_only.category_source,
        Some(CategorySource::Rule),
        "an unchanged Category keeps who set it"
    );

    let recategorised = transactions::update(
        &mut store,
        TransactionEdit {
            category_id: Some(fun.id.clone()),
            ..edit_of(&by_rule)
        },
    )
    .unwrap();
    assert_eq!(recategorised.category_source, Some(CategorySource::User));
}
