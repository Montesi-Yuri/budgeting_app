//! Categorisation Rules (CONTEXT.md "Rule", sync-design §1.3 `rule`, §1.5 step 2).

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::imports::{self, ImportProfile};
use budget_core::rules::{self, Direction, NewRule, RuleEdit};
use budget_core::transactions::{self, CategorySource, TransactionView};
use budget_core::{settings, Error, Store};

fn store() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    settings::set_currency(&mut store, "EUR").unwrap();
    store
}

fn account(store: &mut Store, name: &str) -> Account {
    accounts::create(
        store,
        NewAccount {
            name: name.into(),
            kind: AccountKind::Current,
            opening_minor: 0,
            opening_date: "2026-10-31".into(),
            is_main: false,
        },
    )
    .unwrap()
}

fn category(store: &mut Store, name: &str) -> Category {
    categories::create(
        store,
        NewCategory {
            name: name.into(),
            kind: CategoryKind::Expense,
            parent_id: None,
        },
    )
    .unwrap()
}

fn contains(text: &str, category: &Category) -> NewRule {
    NewRule {
        match_text: text.into(),
        account_id: None,
        direction: None,
        category_id: category.id.clone(),
    }
}

/// Give `account` a simple profile (`date,description,amount`, ISO dates,
/// decimal point) and import `lines` with it.
fn import(store: &mut Store, account: &Account, lines: &str) {
    if imports::profile_for_account(store, &account.id)
        .unwrap()
        .is_none()
    {
        imports::save_profile(
            store,
            ImportProfile {
                name: "CSV".into(),
                delimiter: ",".into(),
                date_column: "date".into(),
                date_format: "%Y-%m-%d".into(),
                amount_column: Some("amount".into()),
                decimal_sep: ".".into(),
                description_columns: vec!["description".into()],
                ..ImportProfile::draft(&account.id)
            },
        )
        .unwrap();
    }
    let csv = format!("date,description,amount\n{lines}");
    imports::import_file(store, &account.id, "bank.csv", csv.as_bytes()).unwrap();
}

/// The Account's Transactions, oldest first, as (description, Category, who set it).
fn categorised(
    store: &Store,
    account: &Account,
) -> Vec<(String, Option<String>, Option<CategorySource>)> {
    let mut list: Vec<TransactionView> =
        transactions::list_for_account(store, &account.id).unwrap();
    list.reverse();
    list.into_iter()
        .map(|t| {
            (
                t.transaction.description,
                t.category_name,
                t.transaction.category_source,
            )
        })
        .collect()
}

fn by_rule(description: &str, category: &str) -> (String, Option<String>, Option<CategorySource>) {
    (
        description.into(),
        Some(category.into()),
        Some(CategorySource::Rule),
    )
}

fn uncategorised(description: &str) -> (String, Option<String>, Option<CategorySource>) {
    (description.into(), None, None)
}

fn texts(store: &Store) -> Vec<String> {
    rules::list(store)
        .unwrap()
        .into_iter()
        .map(|r| r.rule.match_text)
        .collect()
}

#[test]
fn created_rules_are_listed_in_creation_order() {
    let mut store = store();
    let groceries = category(&mut store, "Groceries");
    let current = account(&mut store, "Current");

    rules::create(&mut store, contains("ESSELUNGA", &groceries)).unwrap();
    let coop = rules::create(
        &mut store,
        NewRule {
            account_id: Some(current.id.clone()),
            direction: Some(Direction::Out),
            ..contains("  coop ", &groceries)
        },
    )
    .unwrap();

    assert_eq!(texts(&store), ["ESSELUNGA", "coop"]);
    let listed = rules::list(&store).unwrap();
    assert_eq!(listed[1].rule, coop);
    assert_eq!(coop.account_id.as_deref(), Some(current.id.as_str()));
    assert_eq!(coop.direction, Some(Direction::Out));
    assert_eq!(listed[1].category_name.as_deref(), Some("Groceries"));
    assert!(!listed[1].broken);
}

#[test]
fn a_rule_needs_text_and_an_existing_category_and_account() {
    let mut store = store();
    let groceries = category(&mut store, "Groceries");

    let blank = rules::create(&mut store, contains("  ", &groceries));
    let no_category = rules::create(
        &mut store,
        NewRule {
            category_id: "missing".into(),
            ..contains("coop", &groceries)
        },
    );
    let no_account = rules::create(
        &mut store,
        NewRule {
            account_id: Some("missing".into()),
            ..contains("coop", &groceries)
        },
    );

    assert!(matches!(blank, Err(Error::Invalid(_))));
    assert!(matches!(no_category, Err(Error::Invalid(_))));
    assert!(matches!(no_account, Err(Error::Invalid(_))));
    assert!(rules::list(&store).unwrap().is_empty());
}

#[test]
fn on_import_the_first_matching_rule_assigns_its_category_ignoring_case() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let fuel = category(&mut store, "Fuel");
    rules::create(&mut store, contains("esselunga", &groceries)).unwrap();
    rules::create(&mut store, contains("ESSELUNGA CARBURANTI", &fuel)).unwrap();
    rules::create(&mut store, contains("eni", &fuel)).unwrap();

    import(
        &mut store,
        &current,
        "2026-11-03,Esselunga Carburanti Milano,-40.00\n\
         2026-11-04,ENI station 12,-30.00\n\
         2026-11-05,Amazon,-9.99\n",
    );

    assert_eq!(
        categorised(&store, &current),
        [
            by_rule("Esselunga Carburanti Milano", "Groceries"),
            by_rule("ENI station 12", "Fuel"),
            uncategorised("Amazon"),
        ]
    );
}

#[test]
fn a_rule_limited_to_an_account_or_a_direction_matches_only_those_transactions() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let card = account(&mut store, "Card");
    let groceries = category(&mut store, "Groceries");
    let refunds = category(&mut store, "Refunds");
    rules::create(
        &mut store,
        NewRule {
            account_id: Some(card.id.clone()),
            ..contains("coop", &groceries)
        },
    )
    .unwrap();
    rules::create(
        &mut store,
        NewRule {
            direction: Some(Direction::In),
            ..contains("amazon", &refunds)
        },
    )
    .unwrap();

    let lines = "2026-11-03,COOP,-12.00\n2026-11-04,Amazon,-9.99\n2026-11-05,Amazon,9.99\n";
    import(&mut store, &current, lines);
    import(&mut store, &card, lines);

    assert_eq!(
        categorised(&store, &current),
        [
            uncategorised("COOP"),
            uncategorised("Amazon"),
            by_rule("Amazon", "Refunds"),
        ]
    );
    assert_eq!(
        categorised(&store, &card),
        [
            by_rule("COOP", "Groceries"),
            uncategorised("Amazon"),
            by_rule("Amazon", "Refunds"),
        ]
    );
}

#[test]
fn importing_again_never_changes_a_category_the_user_set() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let gifts = category(&mut store, "Gifts");
    let lines = "2026-11-03,COOP,-12.00\n";
    import(&mut store, &current, lines);
    let txn = transactions::list_for_account(&store, &current.id).unwrap()[0]
        .transaction
        .clone();
    transactions::set_category(&mut store, &txn.id, Some(&gifts.id)).unwrap();
    rules::create(&mut store, contains("coop", &groceries)).unwrap();

    import(&mut store, &current, lines);

    assert_eq!(
        categorised(&store, &current),
        [(
            "COOP".to_owned(),
            Some("Gifts".to_owned()),
            Some(CategorySource::User)
        )]
    );
}

#[test]
fn a_rule_whose_category_was_deleted_is_broken_and_not_applied() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let old = category(&mut store, "Old");
    let groceries = category(&mut store, "Groceries");
    rules::create(&mut store, contains("coop", &old)).unwrap();
    rules::create(&mut store, contains("coop", &groceries)).unwrap();
    categories::delete(&mut store, &old.id).unwrap();

    import(&mut store, &current, "2026-11-03,COOP,-12.00\n");

    let listed = rules::list(&store).unwrap();
    assert_eq!(
        listed.iter().map(|r| r.broken).collect::<Vec<_>>(),
        [true, false]
    );
    assert_eq!(listed[0].category_name, None);
    assert_eq!(
        categorised(&store, &current),
        [by_rule("COOP", "Groceries")]
    );
}

#[test]
fn moving_a_rule_writes_only_that_rule_and_changes_which_rule_wins() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let fuel = category(&mut store, "Fuel");
    rules::create(&mut store, contains("esselunga", &groceries)).unwrap();
    rules::create(&mut store, contains("amazon", &groceries)).unwrap();
    let carburanti = rules::create(&mut store, contains("carburanti", &fuel)).unwrap();
    let before = store.ops_since(store.device_id(), 0).unwrap().len() as u64;

    rules::move_to(&mut store, &carburanti.id, 0).unwrap();
    assert_eq!(texts(&store), ["carburanti", "esselunga", "amazon"]);
    rules::move_to(&mut store, &carburanti.id, 1).unwrap();
    assert_eq!(texts(&store), ["esselunga", "carburanti", "amazon"]);
    rules::move_to(&mut store, &carburanti.id, 99).unwrap();
    assert_eq!(texts(&store), ["esselunga", "amazon", "carburanti"]);
    rules::move_to(&mut store, &carburanti.id, 0).unwrap();

    let moves = store.ops_since(store.device_id(), before).unwrap();
    assert_eq!(moves.len(), 4);
    assert!(moves.iter().all(|op| op.entity_id == carburanti.id));

    import(
        &mut store,
        &current,
        "2026-11-03,Esselunga Carburanti,-40.00\n",
    );
    assert_eq!(
        categorised(&store, &current),
        [by_rule("Esselunga Carburanti", "Fuel")]
    );
}

#[test]
fn a_rule_can_be_edited_and_deleted() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let fuel = category(&mut store, "Fuel");
    let first = rules::create(&mut store, contains("coop", &groceries)).unwrap();
    let second = rules::create(&mut store, contains("eni", &fuel)).unwrap();

    let edited = rules::update(
        &mut store,
        RuleEdit {
            id: first.id.clone(),
            match_text: " esso ".into(),
            account_id: Some(current.id.clone()),
            direction: Some(Direction::Out),
            category_id: fuel.id.clone(),
        },
    )
    .unwrap();
    rules::delete(&mut store, &second.id).unwrap();

    assert_eq!(edited.match_text, "esso");
    assert_eq!(edited.position, first.position);
    let listed = rules::list(&store).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].rule, edited);
    assert_eq!(listed[0].category_name.as_deref(), Some("Fuel"));
    assert_eq!(listed[0].account_name.as_deref(), Some("Current"));
    assert!(matches!(
        rules::delete(&mut store, &second.id),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn picking_a_live_category_repairs_a_broken_rule() {
    let mut store = store();
    let old = category(&mut store, "Old");
    let groceries = category(&mut store, "Groceries");
    let rule = rules::create(&mut store, contains("coop", &old)).unwrap();
    categories::delete(&mut store, &old.id).unwrap();

    rules::update(
        &mut store,
        RuleEdit {
            id: rule.id.clone(),
            match_text: rule.match_text.clone(),
            account_id: None,
            direction: None,
            category_id: groceries.id.clone(),
        },
    )
    .unwrap();

    assert!(!rules::list(&store).unwrap()[0].broken);
}

fn descriptions(list: &[TransactionView]) -> Vec<&str> {
    list.iter()
        .map(|t| t.transaction.description.as_str())
        .collect()
}

#[test]
fn a_new_rule_lists_the_existing_transactions_it_would_recategorise_and_changes_them_only_when_confirmed(
) {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let food = category(&mut store, "Food");
    let gifts = category(&mut store, "Gifts");
    rules::create(&mut store, contains("coop", &food)).unwrap();
    import(
        &mut store,
        &current,
        "2026-11-01,COOP Milano,-10.00\n\
         2026-11-02,Coop online,-11.00\n\
         2026-11-03,coop gift card,-12.00\n\
         2026-11-04,Amazon,-13.00\n",
    );
    let gift = transactions::list_for_account(&store, &current.id)
        .unwrap()
        .into_iter()
        .find(|t| t.transaction.description == "coop gift card")
        .unwrap();
    transactions::set_category(&mut store, &gift.transaction.id, Some(&gifts.id)).unwrap();
    let food_rule = rules::list(&store).unwrap()[0].rule.clone();
    rules::delete(&mut store, &food_rule.id).unwrap();
    let rule = rules::create(&mut store, contains("coop", &groceries)).unwrap();

    let preview = rules::recategorise_preview(&store, &rule.id).unwrap();

    assert_eq!(descriptions(&preview), ["Coop online", "COOP Milano"]);
    assert_eq!(
        categorised(&store, &current)[0],
        by_rule("COOP Milano", "Food")
    );

    let ids: Vec<String> = preview.iter().map(|t| t.transaction.id.clone()).collect();
    let changed = rules::recategorise(&mut store, &rule.id, &ids).unwrap();

    assert_eq!(changed, 2);
    assert_eq!(
        categorised(&store, &current),
        [
            by_rule("COOP Milano", "Groceries"),
            by_rule("Coop online", "Groceries"),
            (
                "coop gift card".to_owned(),
                Some("Gifts".to_owned()),
                Some(CategorySource::User)
            ),
            uncategorised("Amazon"),
        ]
    );
    assert!(rules::recategorise_preview(&store, &rule.id)
        .unwrap()
        .is_empty());
}

#[test]
fn the_recategorise_list_leaves_out_transactions_an_earlier_rule_wins() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let fuel = category(&mut store, "Fuel");
    import(
        &mut store,
        &current,
        "2026-11-01,Esselunga,-10.00\n2026-11-02,Esselunga Carburanti,-40.00\n",
    );
    rules::create(&mut store, contains("carburanti", &fuel)).unwrap();
    let rule = rules::create(&mut store, contains("esselunga", &groceries)).unwrap();

    let preview = rules::recategorise_preview(&store, &rule.id).unwrap();

    assert_eq!(descriptions(&preview), ["Esselunga"]);
}

#[test]
fn confirming_never_overwrites_a_category_the_user_set_after_the_list_was_shown() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let groceries = category(&mut store, "Groceries");
    let gifts = category(&mut store, "Gifts");
    import(&mut store, &current, "2026-11-01,COOP,-10.00\n");
    let rule = rules::create(&mut store, contains("coop", &groceries)).unwrap();
    let preview = rules::recategorise_preview(&store, &rule.id).unwrap();
    let id = preview[0].transaction.id.clone();

    transactions::set_category(&mut store, &id, Some(&gifts.id)).unwrap();
    let changed = rules::recategorise(&mut store, &rule.id, &[id]).unwrap();

    assert_eq!(changed, 0);
    assert_eq!(
        categorised(&store, &current),
        [(
            "COOP".to_owned(),
            Some("Gifts".to_owned()),
            Some(CategorySource::User)
        )]
    );
}

#[test]
fn a_broken_rule_recategorises_nothing() {
    let mut store = store();
    let current = account(&mut store, "Current");
    let old = category(&mut store, "Old");
    import(&mut store, &current, "2026-11-01,COOP,-10.00\n");
    let rule = rules::create(&mut store, contains("coop", &old)).unwrap();
    categories::delete(&mut store, &old.id).unwrap();

    assert!(rules::recategorise_preview(&store, &rule.id)
        .unwrap()
        .is_empty());
}

mod common;

#[test]
fn a_rule_and_its_moves_reach_the_other_device_through_sync() {
    let mut pc = store();
    let mut phone = Store::open_in_memory().unwrap();
    let groceries = category(&mut pc, "Groceries");
    rules::create(&mut pc, contains("coop", &groceries)).unwrap();
    let esselunga = rules::create(&mut pc, contains("esselunga", &groceries)).unwrap();
    rules::move_to(&mut pc, &esselunga.id, 0).unwrap();

    let (a, b) = common::sync_pair(&mut pc, &mut phone);
    a.unwrap();
    b.unwrap();

    assert_eq!(texts(&phone), ["esselunga", "coop"]);
    assert_eq!(rules::list(&phone).unwrap(), rules::list(&pc).unwrap());
}
