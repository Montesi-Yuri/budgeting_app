//! Matching Manual Transactions on Import (CONTEXT.md "Manual Transaction",
//! "Suggestion"; sync-design §1.3 `manual_match`, §1.5 step 3, §1.6).

mod common;

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::date::Date;
use budget_core::imports::{self, ImportProfile, ImportSummary};
use budget_core::manual_matches::{self, ManualMatchView};
use budget_core::period::Period;
use budget_core::review::{self, LinkStatus, ReviewItem};
use budget_core::transactions::{
    self, CategorySource, NewManualTransaction, Transaction, TxnSource,
};
use budget_core::transfers::{self, TransferOrigin};
use budget_core::{settings, spendable, totals, Store};

fn txn(source: TxnSource, account_id: &str, date: &str, amount_minor: i64) -> Transaction {
    Transaction {
        id: format!("{account_id}-{date}-{amount_minor}"),
        account_id: account_id.into(),
        date: date.into(),
        amount_minor,
        description: "Coffee".into(),
        note: None,
        source,
        import_id: None,
        fingerprint: None,
        balance_after_minor: None,
        category_id: None,
        category_source: None,
    }
}

fn manual_txn(account_id: &str, date: &str, amount_minor: i64) -> Transaction {
    txn(TxnSource::Manual, account_id, date, amount_minor)
}

fn imported_txn(account_id: &str, date: &str, amount_minor: i64) -> Transaction {
    txn(TxnSource::Import, account_id, date, amount_minor)
}

#[test]
fn a_match_is_a_manual_transaction_on_the_same_account_with_the_same_amount_give_or_take_one_minor_unit(
) {
    let imported = imported_txn("card", "2026-11-10", -1_250);
    let matches = |m: &Transaction| manual_matches::could_match(m, &imported);
    assert!(matches(&manual_txn("card", "2026-11-10", -1_250)));
    assert!(matches(&manual_txn("card", "2026-11-10", -1_249)));
    assert!(matches(&manual_txn("card", "2026-11-10", -1_251)));
    assert!(!matches(&manual_txn("card", "2026-11-10", -1_252)));
    assert!(!matches(&manual_txn("card", "2026-11-10", 1_250)));
    assert!(!matches(&manual_txn("current", "2026-11-10", -1_250)));
    // Only a Manual Transaction can be matched, and only with an imported one.
    assert!(!matches(&imported_txn("card", "2026-11-10", -1_250)));
    assert!(!manual_matches::could_match(
        &manual_txn("card", "2026-11-10", -1_250),
        &manual_txn("card", "2026-11-10", -1_250)
    ));
}

#[test]
fn a_match_is_dated_at_most_4_days_from_the_imported_transaction() {
    let imported = imported_txn("card", "2026-11-10", -1_250);
    let on = |date: &str| manual_matches::could_match(&manual_txn("card", date, -1_250), &imported);
    assert!(on("2026-11-06"));
    assert!(on("2026-11-14"));
    assert!(!on("2026-11-05"));
    assert!(!on("2026-11-15"));
    // Across a month boundary.
    let imported = imported_txn("card", "2026-12-02", -1_250);
    assert!(manual_matches::could_match(
        &manual_txn("card", "2026-11-28", -1_250),
        &imported
    ));
}

#[test]
fn merging_keeps_the_imported_amount_and_date_and_moves_the_users_category_and_note() {
    let manual = Transaction {
        description: "Coffee with Ana".into(),
        note: Some("my treat".into()),
        category_id: Some("restaurants".into()),
        category_source: Some(CategorySource::User),
        ..manual_txn("card", "2026-11-09", -1_249)
    };
    let imported = Transaction {
        description: "BAR CENTRALE MILANO".into(),
        import_id: Some("batch".into()),
        fingerprint: Some("fp".into()),
        category_id: Some("groceries".into()),
        category_source: Some(CategorySource::Rule),
        ..imported_txn("card", "2026-11-10", -1_250)
    };

    let merged = manual_matches::merged(&manual, &imported);

    assert_eq!(
        merged,
        Transaction {
            id: "card-2026-11-10--1250".into(),
            account_id: "card".into(),
            date: "2026-11-10".into(),
            amount_minor: -1_250,
            description: "BAR CENTRALE MILANO".into(),
            note: Some("my treat".into()),
            source: TxnSource::Import,
            import_id: Some("batch".into()),
            fingerprint: Some("fp".into()),
            balance_after_minor: None,
            category_id: Some("restaurants".into()),
            category_source: Some(CategorySource::User),
        }
    );
}

#[test]
fn merging_an_uncategorised_manual_transaction_without_a_note_keeps_the_imported_ones() {
    let manual = manual_txn("card", "2026-11-09", -1_250);
    let imported = Transaction {
        note: Some("bank note".into()),
        category_id: Some("groceries".into()),
        category_source: Some(CategorySource::Rule),
        ..imported_txn("card", "2026-11-10", -1_250)
    };

    let merged = manual_matches::merged(&manual, &imported);

    assert_eq!(merged, imported);
}

fn store() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    settings::set_currency(&mut store, "EUR").unwrap();
    store
}

fn account(store: &mut Store, name: &str, kind: AccountKind) -> Account {
    accounts::create(
        store,
        NewAccount {
            name: name.into(),
            kind,
            opening_minor: 0,
            opening_date: "2026-09-30".into(),
            is_main: false,
        },
    )
    .unwrap()
}

fn expense(store: &mut Store, name: &str) -> Category {
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

/// A Manual Transaction entered by hand, as on the phone.
fn manual(
    store: &mut Store,
    account: &Account,
    date: &str,
    amount_minor: i64,
    description: &str,
) -> Transaction {
    transactions::create_manual(
        store,
        NewManualTransaction {
            account_id: account.id.clone(),
            date: date.into(),
            amount_minor,
            description: description.into(),
            category_id: None,
            note: None,
        },
    )
    .unwrap()
}

/// Give `account` a simple profile (`date,description,amount[,balance]`,
/// ISO dates, decimal point) and import `lines` with it.
fn import_with(store: &mut Store, account: &Account, header: &str, lines: &str) -> ImportSummary {
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
                balance_column: header.contains("balance").then(|| "balance".into()),
                ..ImportProfile::draft(&account.id)
            },
        )
        .unwrap();
    }
    let csv = format!(
        "{header}
{lines}"
    );
    imports::import_file(store, &account.id, "bank.csv", csv.as_bytes()).unwrap()
}

fn import(store: &mut Store, account: &Account, lines: &str) {
    import_with(store, account, "date,description,amount", lines);
}

fn day(s: &str) -> Date {
    s.parse().unwrap()
}

fn uncategorised_in_november(store: &Store) -> i64 {
    totals::for_dates(store, "2026-11-01", "2026-11-30")
        .unwrap()
        .uncategorised_minor
}

/// The suggested matches in the review list as (manual description,
/// imported description).
fn suggested(store: &Store) -> Vec<(String, String)> {
    review::list(store, day("2026-11-20"))
        .unwrap()
        .items
        .into_iter()
        .filter_map(|item| match item {
            ReviewItem::ManualMatchSuggestion { manual_match } => Some((
                manual_match.manual_txn.transaction.description,
                manual_match.imported_txn.transaction.description,
            )),
            _ => None,
        })
        .collect()
}

fn only_suggestion(store: &Store) -> ManualMatchView {
    let mut items = review::list(store, day("2026-11-20")).unwrap().items;
    assert_eq!(items.len(), 1, "{items:?}");
    match items.pop().unwrap() {
        ReviewItem::ManualMatchSuggestion { manual_match } => manual_match,
        other => panic!("not a match Suggestion: {other:?}"),
    }
}

fn pair(manual: &str, imported: &str) -> (String, String) {
    (manual.to_owned(), imported.to_owned())
}

#[test]
fn an_import_suggests_a_match_shown_with_both_versions_in_the_review_list() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    let coffee = manual(&mut store, &card, "2026-11-09", -250, "Coffee with Ana");

    import(
        &mut store,
        &card,
        "2026-11-10,BAR CENTRALE,-2.50
2026-11-11,ESSELUNGA,-30.00
",
    );

    let list = review::list(&store, day("2026-11-20")).unwrap();
    assert_eq!(list.pending_count, 1);
    let suggestion = only_suggestion(&store);
    assert_eq!(suggestion.manual_match.status, LinkStatus::Suggested);
    assert_eq!(suggestion.manual_txn.transaction, coffee);
    assert_eq!(
        suggestion.imported_txn.transaction.description,
        "BAR CENTRALE"
    );
    assert_eq!(suggestion.imported_txn.transaction.date, "2026-11-10");
    assert_eq!(suggestion.manual_match.manual_txn_id, coffee.id);
    assert_eq!(
        suggestion.manual_match.imported_txn_id,
        suggestion.imported_txn.transaction.id
    );
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
fn while_a_match_is_pending_only_the_imported_transaction_counts() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");
    assert_eq!(balance(&store, &card), -250);

    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.51\n");

    assert_eq!(suggested(&store), vec![pair("Coffee", "BAR CENTRALE")]);
    assert_eq!(balance(&store, &card), -251);
    assert_eq!(uncategorised_in_november(&store), -251);
    let spendable = spendable::for_period(
        &store,
        Period::containing(day("2026-11-15")),
        day("2026-11-15"),
    )
    .unwrap();
    assert_eq!(spendable.spendable_minor, -251);
    let uncategorised: Vec<String> = transactions::list_uncategorised(&store)
        .unwrap()
        .into_iter()
        .map(|t| t.transaction.description)
        .collect();
    assert_eq!(uncategorised, vec!["BAR CENTRALE"]);
}

#[test]
fn a_pending_match_keeps_the_manual_transaction_out_of_budgets() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    let bars = expense(&mut store, "Bars");
    let coffee = manual(&mut store, &card, "2026-11-09", -250, "Coffee");
    transactions::set_category(&mut store, &coffee.id, Some(&bars.id)).unwrap();
    assert_eq!(
        totals::for_dates(&store, "2026-11-01", "2026-11-30")
            .unwrap()
            .spending_minor(&bars.id),
        250
    );

    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.50\n");

    let totals = totals::for_dates(&store, "2026-11-01", "2026-11-30").unwrap();
    assert_eq!(totals.spending_minor(&bars.id), 0);
    assert_eq!(totals.uncategorised_minor, -250);
}

#[test]
fn the_import_balance_check_ignores_a_matched_manual_transaction() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");

    let summary = import_with(
        &mut store,
        &card,
        "date,description,amount,balance",
        "2026-11-10,BAR CENTRALE,-2.50,-2.50\n2026-11-12,ESSELUNGA,-10.00,-12.50\n",
    );

    assert_eq!(summary.balance_mismatch, None);
}

#[test]
fn confirming_moves_the_category_and_note_onto_the_imported_transaction_and_deletes_the_manual_one()
{
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    let bars = expense(&mut store, "Bars");
    let coffee = transactions::create_manual(
        &mut store,
        NewManualTransaction {
            account_id: card.id.clone(),
            date: "2026-11-09".into(),
            amount_minor: -250,
            description: "Coffee with Ana".into(),
            category_id: Some(bars.id.clone()),
            note: Some("my treat".into()),
        },
    )
    .unwrap();
    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.51\n");
    let suggestion = only_suggestion(&store);

    let confirmed = manual_matches::confirm(&mut store, &suggestion.manual_match.id).unwrap();

    assert_eq!(confirmed.status, LinkStatus::Confirmed);
    let list = review::list(&store, day("2026-11-20")).unwrap();
    assert_eq!(list.pending_count, 0);
    assert!(list.items.is_empty());
    let left: Vec<Transaction> = transactions::list_for_account(&store, &card.id)
        .unwrap()
        .into_iter()
        .map(|v| v.transaction)
        .collect();
    assert_eq!(left.len(), 1, "the Manual Transaction is deleted");
    let merged = &left[0];
    assert_eq!(merged.id, suggestion.imported_txn.transaction.id);
    assert_ne!(merged.id, coffee.id);
    assert_eq!(merged.date, "2026-11-10");
    assert_eq!(merged.amount_minor, -251);
    assert_eq!(merged.description, "BAR CENTRALE");
    assert_eq!(merged.note.as_deref(), Some("my treat"));
    assert_eq!(merged.category_id.as_deref(), Some(bars.id.as_str()));
    assert_eq!(merged.category_source, Some(CategorySource::User));
    assert_eq!(balance(&store, &card), -251);
    let totals = totals::for_dates(&store, "2026-11-01", "2026-11-30").unwrap();
    assert_eq!(totals.spending_minor(&bars.id), 251);
}

#[test]
fn rejecting_keeps_both_transactions_and_counts_them_both_again() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");
    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    let suggestion = only_suggestion(&store);

    let rejected = manual_matches::reject(&mut store, &suggestion.manual_match.id).unwrap();

    assert_eq!(rejected.status, LinkStatus::Rejected);
    assert_eq!(
        review::list(&store, day("2026-11-20"))
            .unwrap()
            .pending_count,
        0
    );
    assert_eq!(
        transactions::list_for_account(&store, &card.id)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(balance(&store, &card), -500);
    assert_eq!(uncategorised_in_november(&store), -500);
}

#[test]
fn a_rejected_pair_is_never_suggested_again() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");
    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    let id = only_suggestion(&store).manual_match.id;
    manual_matches::reject(&mut store, &id).unwrap();

    // The same file again, and a later one overlapping it.
    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    import(
        &mut store,
        &card,
        "2026-11-10,BAR CENTRALE,-2.50\n2026-11-11,ESSELUNGA,-30.00\n",
    );

    assert!(suggested(&store).is_empty());
    // The rejected Manual Transaction can still match another one.
    import(&mut store, &card, "2026-11-12,BAR STAZIONE,-2.50\n");
    assert_eq!(suggested(&store), vec![pair("Coffee", "BAR STAZIONE")]);
}

#[test]
fn a_decided_match_cant_be_decided_again() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");
    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    let id = only_suggestion(&store).manual_match.id;
    manual_matches::confirm(&mut store, &id).unwrap();

    assert!(manual_matches::reject(&mut store, &id).is_err());
    assert!(manual_matches::confirm(&mut store, &id).is_err());
    assert!(manual_matches::confirm(&mut store, "no-such-match").is_err());
}

#[test]
fn a_manual_transaction_already_matched_is_not_suggested_for_another_import() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");

    import(
        &mut store,
        &card,
        "2026-11-10,BAR CENTRALE,-2.50\n2026-11-11,BAR STAZIONE,-2.50\n",
    );
    import(&mut store, &card, "2026-11-12,BAR PIAZZA,-2.50\n");

    assert_eq!(suggested(&store), vec![pair("Coffee", "BAR CENTRALE")]);
}

#[test]
fn each_imported_transaction_is_matched_with_the_closest_manual_transaction() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    let current = account(&mut store, "Current", AccountKind::Current);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee Monday");
    manual(&mut store, &card, "2026-11-12", -250, "Coffee Thursday");
    manual(&mut store, &card, "2026-11-12", -251, "Coffee rounded");
    manual(
        &mut store,
        &current,
        "2026-11-12",
        -250,
        "Coffee on the other Account",
    );

    import(
        &mut store,
        &card,
        "2026-11-10,BAR CENTRALE,-2.50\n2026-11-13,BAR STAZIONE,-2.50\n",
    );

    let mut got = suggested(&store);
    got.sort();
    assert_eq!(
        got,
        vec![
            pair("Coffee Monday", "BAR CENTRALE"),
            pair("Coffee Thursday", "BAR STAZIONE"),
        ]
    );
}

fn sync(a: &mut Store, b: &mut Store) {
    let (x, y) = common::sync_pair(a, b);
    x.unwrap();
    y.unwrap();
}

#[test]
fn the_same_match_made_on_two_devices_is_one_item_after_sync() {
    let mut phone = store();
    let card = account(&mut phone, "Card", AccountKind::CreditCard);
    manual(&mut phone, &card, "2026-11-09", -250, "Coffee");
    let mut pc = Store::open_in_memory().unwrap();
    sync(&mut pc, &mut phone);

    // Both Devices import the same card file before syncing.
    import(&mut pc, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    import(&mut phone, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    let on_pc = only_suggestion(&pc);
    let on_phone = only_suggestion(&phone);
    assert_eq!(on_pc.manual_match.id, on_phone.manual_match.id);
    // The phone rejects its Suggestion; the later decision wins.
    manual_matches::reject(&mut phone, &on_phone.manual_match.id).unwrap();

    sync(&mut pc, &mut phone);

    assert_eq!(common::synced_rows(&pc), common::synced_rows(&phone));
    let rows: i64 = pc
        .conn()
        .query_row("SELECT COUNT(*) FROM manual_match", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1);
    assert!(review::list(&pc, day("2026-11-20"))
        .unwrap()
        .items
        .is_empty());
    assert_eq!(balance(&pc, &card), -500);
}

#[test]
fn a_merge_confirmed_on_one_device_reaches_the_other() {
    let mut phone = store();
    let card = account(&mut phone, "Card", AccountKind::CreditCard);
    let bars = expense(&mut phone, "Bars");
    let coffee = manual(&mut phone, &card, "2026-11-09", -250, "Coffee");
    transactions::set_category(&mut phone, &coffee.id, Some(&bars.id)).unwrap();
    let mut pc = Store::open_in_memory().unwrap();
    sync(&mut pc, &mut phone);
    import(&mut pc, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    let id = only_suggestion(&pc).manual_match.id;
    manual_matches::confirm(&mut pc, &id).unwrap();

    sync(&mut pc, &mut phone);

    assert_eq!(common::synced_rows(&pc), common::synced_rows(&phone));
    let on_phone = transactions::list_for_account(&phone, &card.id).unwrap();
    assert_eq!(on_phone.len(), 1);
    assert_eq!(on_phone[0].transaction.description, "BAR CENTRALE");
    assert_eq!(on_phone[0].category_name.as_deref(), Some("Bars"));
    assert_eq!(balance(&phone, &card), -250);
}

/// The Transfers as (out description, in description).
fn transfer_pairs(store: &Store) -> Vec<(String, String)> {
    let name = |t: &Option<transactions::TransactionView>| {
        t.as_ref()
            .map_or("-".to_owned(), |t| t.transaction.description.clone())
    };
    transfers::list(store)
        .unwrap()
        .iter()
        .map(|t| (name(&t.out_txn), name(&t.in_txn)))
        .collect()
}

#[test]
fn a_manual_transaction_hidden_by_a_match_is_not_the_other_side_of_a_transfer() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    manual(
        &mut store,
        &current,
        "2026-11-05",
        -10_000,
        "To savings (by hand)",
    );
    import(&mut store, &current, "2026-11-07,BONIFICO,-100.00\n");
    assert_eq!(
        suggested(&store),
        vec![pair("To savings (by hand)", "BONIFICO")]
    );

    import(&mut store, &savings, "2026-11-05,FROM CURRENT,100.00\n");

    assert_eq!(
        transfer_pairs(&store),
        vec![pair("BONIFICO", "FROM CURRENT")]
    );
    assert_eq!(uncategorised_in_november(&store), 0);
}

#[test]
fn confirming_a_match_moves_the_manual_transactions_transfer_onto_the_imported_one() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    import(&mut store, &savings, "2026-11-05,FROM CURRENT,100.00\n");
    let by_hand = manual(&mut store, &current, "2026-11-05", -10_000, "To savings");
    let into = transactions::list_for_account(&store, &savings.id).unwrap()[0]
        .transaction
        .clone();
    transfers::link(&mut store, &by_hand.id, &into.id).unwrap();
    import(&mut store, &current, "2026-11-06,BONIFICO,-100.00\n");
    let id = only_suggestion(&store).manual_match.id;

    manual_matches::confirm(&mut store, &id).unwrap();

    assert_eq!(
        transfer_pairs(&store),
        vec![pair("BONIFICO", "FROM CURRENT")]
    );
    let moved = &transfers::list(&store).unwrap()[0].transfer;
    assert_eq!(moved.status, LinkStatus::Confirmed);
    assert_eq!(moved.origin, TransferOrigin::User);
    assert_eq!(uncategorised_in_november(&store), 0);
    assert_eq!(balance(&store, &current), -10_000);
    assert!(review::list(&store, day("2026-11-20"))
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn deleting_the_imported_transaction_of_a_pending_match_makes_the_manual_one_count_again() {
    let mut store = store();
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    manual(&mut store, &card, "2026-11-09", -250, "Coffee");
    import(&mut store, &card, "2026-11-10,BAR CENTRALE,-2.50\n");
    let imported = only_suggestion(&store).imported_txn.transaction;

    transactions::delete(&mut store, &imported.id).unwrap();

    assert!(review::list(&store, day("2026-11-20"))
        .unwrap()
        .items
        .is_empty());
    assert_eq!(balance(&store, &card), -250);
    assert_eq!(uncategorised_in_november(&store), -250);
}
