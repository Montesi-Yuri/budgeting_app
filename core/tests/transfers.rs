//! Transfers and the review list (CONTEXT.md "Transfer", "Suggestion";
//! sync-design §1.3 `transfer`, §1.5 step 3, §1.7).

mod common;

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::categories::{self, Category, CategoryKind, NewCategory};
use budget_core::date::Date;
use budget_core::imports::{self, ImportProfile};
use budget_core::period::Period;
use budget_core::review::{self, LinkStatus};
use budget_core::rules::{self, NewRule, RuleAction};
use budget_core::transactions::{self, NewManualTransaction, Transaction};
use budget_core::transfers::{self, TransferOrigin, TransferView};
use budget_core::{settings, spendable, totals, Store};

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

fn manual(store: &mut Store, account: &Account, date: &str, amount_minor: i64) -> Transaction {
    transactions::create_manual(
        store,
        NewManualTransaction {
            account_id: account.id.clone(),
            date: date.into(),
            amount_minor,
            description: "Manual".into(),
            category_id: None,
            note: None,
        },
    )
    .unwrap()
}

fn day(s: &str) -> Date {
    s.parse().unwrap()
}

fn november() -> Period {
    Period::containing(day("2026-11-15"))
}

fn uncategorised_in_november(store: &Store) -> i64 {
    totals::for_dates(store, "2026-11-01", "2026-11-30")
        .unwrap()
        .uncategorised_minor
}

#[test]
fn transactions_linked_as_a_transfer_are_left_out_of_the_totals_and_spendable() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    // Sent on the last day of October, arrived in November.
    let out = manual(&mut store, &current, "2026-10-31", -50_000);
    let into = manual(&mut store, &savings, "2026-11-02", 50_000);
    manual(&mut store, &current, "2026-11-04", -1_200);
    assert_eq!(uncategorised_in_november(&store), 48_800);

    transfers::link(&mut store, &out.id, &into.id).unwrap();

    assert_eq!(uncategorised_in_november(&store), -1_200);
    let spendable = spendable::for_period(&store, november(), day("2026-11-15")).unwrap();
    assert_eq!(spendable.uncategorised_minor, -1_200);
    assert_eq!(spendable.spendable_minor, -1_200);
}

fn txn(account_id: &str, date: &str, amount_minor: i64) -> Transaction {
    Transaction {
        id: format!("{account_id}-{date}-{amount_minor}"),
        account_id: account_id.into(),
        date: date.into(),
        amount_minor,
        description: "Bonifico".into(),
        note: None,
        source: transactions::TxnSource::Import,
        import_id: None,
        fingerprint: None,
        balance_after_minor: None,
        category_id: None,
        category_source: None,
    }
}

#[test]
fn a_transfer_pair_has_the_same_amount_with_opposite_signs_on_different_accounts() {
    let out = txn("current", "2026-11-10", -30_000);
    assert!(transfers::could_pair(
        &out,
        &txn("card", "2026-11-10", 30_000)
    ));
    assert!(transfers::could_pair(
        &txn("card", "2026-11-10", 30_000),
        &out
    ));
    assert!(!transfers::could_pair(
        &out,
        &txn("card", "2026-11-10", 29_999)
    ));
    assert!(!transfers::could_pair(
        &out,
        &txn("card", "2026-11-10", -30_000)
    ));
    assert!(!transfers::could_pair(
        &out,
        &txn("current", "2026-11-10", 30_000)
    ));
}

#[test]
fn the_two_sides_of_a_transfer_are_at_most_3_days_apart() {
    let out = txn("current", "2026-11-30", -30_000);
    for (date, pairs) in [
        ("2026-11-27", true),
        ("2026-11-26", false),
        ("2026-12-03", true), // across the Period boundary
        ("2026-12-04", false),
    ] {
        assert_eq!(
            transfers::could_pair(&out, &txn("card", date, 30_000)),
            pairs,
            "{date}"
        );
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

/// The Transfers in the review list as (out description, in description),
/// `-` for a missing side.
fn suggested(store: &Store) -> Vec<(String, String)> {
    review::list(store, day("2026-11-20"))
        .unwrap()
        .items
        .into_iter()
        .filter_map(|item| match item {
            review::ReviewItem::TransferSuggestion { transfer } => Some(sides(&transfer)),
            _ => None,
        })
        .collect()
}

fn sides(view: &TransferView) -> (String, String) {
    let name = |t: &Option<transactions::TransactionView>| {
        t.as_ref()
            .map_or("-".to_owned(), |t| t.transaction.description.clone())
    };
    (name(&view.out_txn), name(&view.in_txn))
}

fn pair(out: &str, into: &str) -> (String, String) {
    (out.to_owned(), into.to_owned())
}

#[test]
fn an_import_suggests_transfers_that_count_as_confirmed_until_decided() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    import(&mut store, &savings, "2026-10-31,From current,500.00\n");

    import(
        &mut store,
        &current,
        "2026-11-02,To savings,-500.00\n2026-11-04,Groceries,-12.00\n",
    );

    assert_eq!(suggested(&store), vec![pair("To savings", "From current")]);
    let list = review::list(&store, day("2026-11-20")).unwrap();
    assert_eq!(list.pending_count, 1);
    // Only the groceries count while the Suggestion is pending.
    assert_eq!(uncategorised_in_november(&store), -1_200);
}

fn only_suggestion(store: &Store) -> TransferView {
    let mut items = review::list(store, day("2026-11-20")).unwrap().items;
    assert_eq!(items.len(), 1);
    match items.pop().unwrap() {
        review::ReviewItem::TransferSuggestion { transfer } => transfer,
        other => panic!("not a Transfer Suggestion: {other:?}"),
    }
}

#[test]
fn confirming_a_suggestion_keeps_it_out_of_the_numbers_and_off_the_review_list() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    import(&mut store, &card, "2026-11-01,Payment received,300.00\n");
    import(&mut store, &current, "2026-11-02,Card bill,-300.00\n");
    let suggestion = only_suggestion(&store);

    transfers::confirm(&mut store, &suggestion.transfer.id).unwrap();

    let list = review::list(&store, day("2026-11-20")).unwrap();
    assert_eq!(list.pending_count, 0);
    assert!(list.items.is_empty());
    assert_eq!(uncategorised_in_november(&store), 0);
    let all = transfers::list(&store).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].transfer.status, LinkStatus::Confirmed);
}

#[test]
fn rejecting_a_suggestion_counts_both_transactions_again() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let friend = account(&mut store, "Shared", AccountKind::Current);
    import(&mut store, &friend, "2026-11-01,Dinner share,40.00\n");
    import(&mut store, &current, "2026-11-02,Concert,-40.00\n");
    let suggestion = only_suggestion(&store);

    transfers::reject(&mut store, &suggestion.transfer.id).unwrap();

    assert_eq!(
        review::list(&store, day("2026-11-20"))
            .unwrap()
            .pending_count,
        0
    );
    assert!(transfers::list(&store).unwrap().is_empty());
    assert_eq!(
        totals::for_dates(&store, "2026-11-01", "2026-11-30")
            .unwrap()
            .uncategorised_minor,
        0, // +40 and -40, both counted again
    );
}

#[test]
fn a_rejected_pair_is_never_suggested_again() {
    // Device A imports both sides and rejects the Suggestion.
    let mut a = store();
    let current = account(&mut a, "Current", AccountKind::Current);
    let shared = account(&mut a, "Shared", AccountKind::Current);
    import(&mut a, &current, "2026-11-02,Concert,-40.00\n");
    import(&mut a, &shared, "2026-11-01,Dinner share,40.00\n");
    let suggestion = only_suggestion(&a);
    transfers::reject(&mut a, &suggestion.transfer.id).unwrap();

    // Device B has heard of everything except the shared Account's
    // Transaction, then imports that same file itself.
    let shared_txn = suggestion.in_txn.unwrap().transaction.id;
    let mut b = store();
    let ops: Vec<_> = common::all_ops(&a)
        .into_iter()
        .filter(|op| op.entity_id != shared_txn)
        .collect();
    b.apply_remote(&ops).unwrap();
    import(&mut b, &shared, "2026-11-01,Dinner share,40.00\n");

    assert!(suggested(&b).is_empty());
}

#[test]
fn a_transaction_already_in_a_transfer_is_not_suggested_for_another() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    let other = account(&mut store, "Other", AccountKind::Savings);
    import(&mut store, &savings, "2026-11-02,From current,100.00\n");
    import(&mut store, &current, "2026-11-02,To savings,-100.00\n");
    let first = only_suggestion(&store);
    transfers::confirm(&mut store, &first.transfer.id).unwrap();

    import(&mut store, &other, "2026-11-03,Deposit,100.00\n");

    assert!(suggested(&store).is_empty());
}

fn transfer_rule(store: &mut Store, text: &str) {
    rules::create(
        store,
        NewRule {
            match_text: text.into(),
            account_id: None,
            direction: None,
            action: RuleAction::Transfer,
            category_id: None,
        },
    )
    .unwrap();
}

#[test]
fn a_transfer_rule_makes_a_confirmed_transfer_with_one_side_until_the_other_is_imported() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    transfer_rule(&mut store, "addebito carta");

    import(
        &mut store,
        &current,
        "2026-11-05,ADDEBITO CARTA NOV,-250.00\n2026-11-05,Bakery,-3.00\n",
    );

    let all = transfers::list(&store).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(sides(&all[0]), pair("ADDEBITO CARTA NOV", "-"));
    assert_eq!(all[0].transfer.status, LinkStatus::Confirmed);
    assert_eq!(all[0].transfer.origin, TransferOrigin::Rule);
    assert_eq!(uncategorised_in_november(&store), -300);
    assert_eq!(
        review::list(&store, day("2026-11-20"))
            .unwrap()
            .pending_count,
        0
    );

    // The card's statement arrives: the same Transfer gets its other side.
    import(&mut store, &card, "2026-11-06,Repayment thank you,250.00\n");

    let all = transfers::list(&store).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(
        sides(&all[0]),
        pair("ADDEBITO CARTA NOV", "Repayment thank you")
    );
    assert_eq!(all[0].transfer.status, LinkStatus::Confirmed);
    assert_eq!(uncategorised_in_november(&store), -300);
    assert_eq!(
        review::list(&store, day("2026-11-20"))
            .unwrap()
            .pending_count,
        0
    );
}

#[test]
fn a_transfer_rule_pairs_with_the_other_side_when_it_is_already_there() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    transfer_rule(&mut store, "giroconto");
    import(&mut store, &savings, "2026-11-01,Incoming,1000.00\n");

    import(&mut store, &current, "2026-11-02,GIROCONTO,-1000.00\n");

    let all = transfers::list(&store).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(sides(&all[0]), pair("GIROCONTO", "Incoming"));
    assert_eq!(all[0].transfer.status, LinkStatus::Confirmed);
    assert!(suggested(&store).is_empty());
}

#[test]
fn a_transfer_rule_wins_over_later_categorise_rules_and_keeps_the_category_alone() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let groceries = expense(&mut store, "Groceries");
    transfer_rule(&mut store, "giroconto");
    rules::create(
        &mut store,
        NewRule {
            match_text: "giroconto".into(),
            account_id: None,
            direction: None,
            action: RuleAction::Categorise,
            category_id: Some(groceries.id.clone()),
        },
    )
    .unwrap();

    import(&mut store, &current, "2026-11-02,GIROCONTO,-1000.00\n");

    let txns = transactions::list_for_account(&store, &current.id).unwrap();
    assert_eq!(txns[0].transaction.category_id, None);
    assert_eq!(transfers::list(&store).unwrap().len(), 1);
}

#[test]
fn a_transfer_rule_needs_no_category_and_is_never_broken() {
    let mut store = store();
    transfer_rule(&mut store, "giroconto");

    let listed = rules::list(&store).unwrap();
    assert_eq!(listed[0].rule.action, RuleAction::Transfer);
    assert_eq!(listed[0].rule.category_id, None);
    assert!(!listed[0].broken);
}

#[test]
fn the_user_can_link_two_transactions_by_hand_and_unlink_them() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    // Too far apart, and with a fee, to be suggested.
    let out = manual(&mut store, &current, "2026-11-01", -10_150);
    let into = manual(&mut store, &savings, "2026-11-09", 10_000);

    let linked = transfers::link(&mut store, &into.id, &out.id).unwrap();
    assert_eq!(linked.out_txn_id.as_deref(), Some(out.id.as_str()));
    assert_eq!(linked.in_txn_id.as_deref(), Some(into.id.as_str()));
    assert_eq!(linked.origin, TransferOrigin::User);
    assert_eq!(uncategorised_in_november(&store), 0);

    transfers::reject(&mut store, &linked.id).unwrap();
    assert!(transfers::list(&store).unwrap().is_empty());
    assert_eq!(uncategorised_in_november(&store), -150);

    // Changing their mind again is fine.
    transfers::link(&mut store, &out.id, &into.id).unwrap();
    assert_eq!(uncategorised_in_november(&store), 0);
}

#[test]
fn linking_by_hand_needs_money_out_and_in_on_different_accounts_not_already_paired() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    let out = manual(&mut store, &current, "2026-11-01", -100);
    let out2 = manual(&mut store, &current, "2026-11-01", -100);
    let into = manual(&mut store, &savings, "2026-11-01", 100);
    let into2 = manual(&mut store, &savings, "2026-11-01", 100);

    assert!(transfers::link(&mut store, &out.id, &out2.id).is_err());
    assert!(transfers::link(&mut store, &into.id, &into2.id).is_err());
    transfers::link(&mut store, &out.id, &into.id).unwrap();
    assert!(transfers::link(&mut store, &out.id, &into2.id).is_err());
    assert_eq!(transfers::list(&store).unwrap().len(), 1);
}

#[test]
fn linking_by_hand_completes_a_transfer_that_had_one_side() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    transfer_rule(&mut store, "addebito carta");
    import(&mut store, &current, "2026-11-05,ADDEBITO CARTA,-250.00\n");
    let out = transactions::list_for_account(&store, &current.id).unwrap()[0]
        .transaction
        .clone();
    let into = manual(&mut store, &card, "2026-11-20", 25_000);

    transfers::link(&mut store, &out.id, &into.id).unwrap();

    let all = transfers::list(&store).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(sides(&all[0]), pair("ADDEBITO CARTA", "Manual"));
}

#[test]
fn unlinking_a_completed_rule_transfer_leaves_the_rule_side_a_transfer_on_its_own() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    transfer_rule(&mut store, "addebito carta");
    import(&mut store, &current, "2026-11-05,ADDEBITO CARTA,-250.00\n");
    import(&mut store, &card, "2026-11-06,Refund from shop,250.00\n");
    let completed = transfers::list(&store).unwrap().pop().unwrap();
    assert!(completed.is_complete());

    transfers::reject(&mut store, &completed.transfer.id).unwrap();

    let all = transfers::list(&store).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(sides(&all[0]), pair("ADDEBITO CARTA", "-"));
    assert_eq!(uncategorised_in_november(&store), 25_000); // the refund counts again
}

#[test]
fn a_transfer_with_one_side_at_the_end_of_its_period_is_listed_as_other_side_not_imported_yet() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    transfer_rule(&mut store, "addebito carta");
    import(
        &mut store,
        &current,
        "2026-10-28,ADDEBITO CARTA OTT,-250.00\n2026-11-05,ADDEBITO CARTA NOV,-90.00\n",
    );

    let list = review::list(&store, day("2026-11-20")).unwrap();

    assert_eq!(list.pending_count, 0);
    let missing: Vec<_> = list
        .items
        .iter()
        .map(|item| match item {
            review::ReviewItem::TransferMissingSide { transfer } => sides(transfer),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    // November's is still in its Period, so it isn't listed yet.
    assert_eq!(missing, vec![pair("ADDEBITO CARTA OTT", "-")]);
    // It is still a Transfer.
    let october = totals::for_dates(&store, "2026-10-01", "2026-10-31").unwrap();
    assert_eq!(october.uncategorised_minor, 0);
}

#[test]
fn the_same_suggestion_made_on_two_devices_is_one_item_after_sync() {
    let mut pc = store();
    let current = account(&mut pc, "Current", AccountKind::Current);
    let savings = account(&mut pc, "Savings", AccountKind::Savings);
    import(&mut pc, &savings, "2026-11-01,In,75.00\n");
    let mut phone = Store::open_in_memory().unwrap();
    let (a, b) = common::sync_pair(&mut pc, &mut phone);
    a.unwrap();
    b.unwrap();

    // Both Devices import the same current account file before syncing.
    import(&mut pc, &current, "2026-11-02,Out,-75.00\n");
    import(&mut phone, &current, "2026-11-02,Out,-75.00\n");
    let on_pc = only_suggestion(&pc).transfer;
    // The phone confirms its Suggestion; the later decision wins.
    let on_phone = only_suggestion(&phone).transfer;
    assert_eq!(on_phone.id, on_pc.id);
    transfers::confirm(&mut phone, &on_phone.id).unwrap();

    let (a, b) = common::sync_pair(&mut pc, &mut phone);
    a.unwrap();
    b.unwrap();

    assert_eq!(common::synced_rows(&pc), common::synced_rows(&phone));
    let all = transfers::list(&pc).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].transfer.id, on_pc.id);
    assert_eq!(all[0].transfer.status, LinkStatus::Confirmed);
    assert_eq!(
        review::list(&pc, day("2026-11-20")).unwrap().pending_count,
        0
    );
}

#[test]
fn savings_progress_is_the_target_next_to_what_transfers_moved_into_savings_this_period() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    let card = account(&mut store, "Card", AccountKind::CreditCard);
    settings::set_savings_target(&mut store, 30_000).unwrap();
    let transfer = |store: &mut Store, from: &Account, to: &Account, date: &str, minor: i64| {
        let out = manual(store, from, date, -minor);
        let into = manual(store, to, date, minor);
        transfers::link(store, &out.id, &into.id).unwrap()
    };
    transfer(&mut store, &current, &savings, "2026-11-01", 20_000);
    transfer(&mut store, &current, &savings, "2026-11-30", 5_000);
    transfer(&mut store, &current, &savings, "2026-10-31", 7_000); // last Period
    transfer(&mut store, &savings, &current, "2026-11-10", 1_000); // out of savings
    transfer(&mut store, &current, &card, "2026-11-10", 9_000); // card bill
    let unlinked = transfer(&mut store, &current, &savings, "2026-11-12", 4_000);
    transfers::reject(&mut store, &unlinked.id).unwrap();
    manual(&mut store, &savings, "2026-11-30", 300); // interest, not a Transfer

    let progress = transfers::savings_progress(&store, november()).unwrap();

    assert_eq!(progress.target_minor, 30_000);
    assert_eq!(progress.transferred_minor, 25_000);
}

#[test]
fn transactions_in_a_transfer_need_no_category_so_they_leave_the_uncategorised_list() {
    let mut store = store();
    let current = account(&mut store, "Current", AccountKind::Current);
    let savings = account(&mut store, "Savings", AccountKind::Savings);
    let out = manual(&mut store, &current, "2026-11-01", -100);
    let into = manual(&mut store, &savings, "2026-11-01", 100);
    let coffee = manual(&mut store, &current, "2026-11-01", -2);

    let transfer = transfers::link(&mut store, &out.id, &into.id).unwrap();
    let ids = |store: &Store| -> Vec<String> {
        transactions::list_uncategorised(store)
            .unwrap()
            .into_iter()
            .map(|v| v.transaction.id)
            .collect()
    };
    assert_eq!(ids(&store), vec![coffee.id.clone()]);

    transfers::reject(&mut store, &transfer.id).unwrap();
    assert_eq!(ids(&store).len(), 3);
}
