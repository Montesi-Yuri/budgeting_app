//! CSV Imports with Import profiles (CONTEXT.md, sync-design §1.5).

use budget_core::accounts::{self, Account, AccountKind, NewAccount};
use budget_core::imports::{self, AmountMode, Encoding, ImportProfile};
use budget_core::transactions::{self, TxnSource};
use budget_core::{settings, Error, Store};

/// A Store with the Currency set to EUR.
fn store() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    settings::set_currency(&mut store, "EUR").unwrap();
    store
}

/// An Account with an Opening balance of 1,000.00 on 2026-10-31.
fn checking(store: &mut Store) -> Account {
    accounts::create(
        store,
        NewAccount {
            name: "Checking".into(),
            kind: AccountKind::Current,
            opening_minor: 100_000,
            opening_date: "2026-10-31".into(),
            is_main: true,
        },
    )
    .unwrap()
}

/// An Italian-style layout: `;` separated, decimal comma, `.` thousands,
/// dates as dd/mm/yyyy, one signed amount column.
fn italian_profile(account: &Account) -> ImportProfile {
    ImportProfile {
        account_id: account.id.clone(),
        name: "Bank CSV".into(),
        encoding: Encoding::Utf8,
        delimiter: ";".into(),
        skip_rows: 0,
        date_column: "Data".into(),
        date_format: "%d/%m/%Y".into(),
        amount_mode: AmountMode::Signed,
        amount_column: Some("Importo".into()),
        debit_column: None,
        credit_column: None,
        decimal_sep: ",".into(),
        thousands_sep: Some(".".into()),
        description_columns: vec!["Descrizione".into()],
        balance_column: None,
        ..ImportProfile::draft(&account.id)
    }
}

#[test]
fn the_preview_parses_rows_with_a_decimal_comma_and_thousands_separator() {
    let mut store = store();
    let account = checking(&mut store);
    let csv = "Data;Descrizione;Importo\n\
               03/11/2026;Esselunga;-12,50\n\
               27/11/2026;Stipendio;1.250,00\n";

    let preview = imports::preview(&store, &italian_profile(&account), csv.as_bytes()).unwrap();

    assert_eq!(preview.columns, ["Data", "Descrizione", "Importo"]);
    let parsed: Vec<_> = preview
        .rows
        .iter()
        .map(|r| {
            (
                r.date.as_deref(),
                r.amount_minor,
                r.description.as_str(),
                r.error.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        parsed,
        [
            (Some("2026-11-03"), Some(-1_250), "Esselunga", None),
            (Some("2026-11-27"), Some(125_000), "Stipendio", None),
        ]
    );
}

/// Parsed (date, amount, description, error) of a preview row.
type Row = (Option<String>, Option<i64>, String, Option<String>);

fn preview_rows(store: &Store, profile: &ImportProfile, file: &[u8]) -> Vec<Row> {
    let preview = imports::preview(store, profile, file).unwrap();
    assert_eq!(preview.problem, None);
    preview
        .rows
        .into_iter()
        .map(|r| (r.date, r.amount_minor, r.description, r.error))
        .collect()
}

fn row(
    date: &str,
    amount: i64,
    description: &str,
) -> (Option<String>, Option<i64>, String, Option<String>) {
    (Some(date.into()), Some(amount), description.into(), None)
}

#[test]
fn split_debit_and_credit_columns_give_negative_debits_and_positive_credits() {
    let mut store = store();
    let account = checking(&mut store);
    let profile = ImportProfile {
        delimiter: ",".into(),
        date_column: "Date".into(),
        date_format: "%Y-%m-%d".into(),
        amount_mode: AmountMode::Split,
        amount_column: None,
        debit_column: Some("Debit".into()),
        credit_column: Some("Credit".into()),
        decimal_sep: ".".into(),
        thousands_sep: Some(",".into()),
        description_columns: vec!["Payee".into(), "Memo".into()],
        ..italian_profile(&account)
    };
    // Debits written with or without a minus are both money out.
    let csv = "Date,Payee,Memo,Debit,Credit\n\
               2026-11-02,Coffee Bar,  card 1234 ,3.20,\n\
               2026-11-05,ACME Ltd,Salary,,\"2,500.00\"\n\
               2026-11-06,Rent,,-800,\n";

    assert_eq!(
        preview_rows(&store, &profile, csv.as_bytes()),
        [
            row("2026-11-02", -320, "Coffee Bar card 1234"),
            row("2026-11-05", 250_000, "ACME Ltd Salary"),
            row("2026-11-06", -80_000, "Rent"),
        ]
    );
}

#[test]
fn lines_before_the_header_are_skipped_and_windows_1252_text_is_decoded() {
    let mut store = store();
    let account = checking(&mut store);
    let profile = ImportProfile {
        encoding: Encoding::Windows1252,
        skip_rows: 2,
        ..italian_profile(&account)
    };
    // "Caffè" with è as the single Windows-1252 byte 0xE8.
    let mut file = b"Estratto conto\nConto 1234\nData;Descrizione;Importo\n".to_vec();
    file.extend_from_slice(b"04/11/2026;Caff\xE8 Roma;-1,10\n");

    assert_eq!(
        preview_rows(&store, &profile, &file),
        [row("2026-11-04", -110, "Caffè Roma")]
    );
}

#[test]
fn the_preview_shows_why_a_line_cant_be_read_and_names_a_missing_column() {
    let mut store = store();
    let account = checking(&mut store);
    let csv = "Data;Descrizione;Importo\n\
               31/02/2026;Bad date;-1,00\n\
               01/11/2026;Bad amount;12.50.1\n";

    let preview = imports::preview(&store, &italian_profile(&account), csv.as_bytes()).unwrap();
    assert_eq!(preview.rows_with_errors, 2);
    assert!(preview.rows[0]
        .error
        .as_deref()
        .unwrap()
        .contains("31/02/2026"));
    assert!(preview.rows[1]
        .error
        .as_deref()
        .unwrap()
        .contains("12.50.1"));

    let wrong_column = ImportProfile {
        amount_column: Some("Amount".into()),
        ..italian_profile(&account)
    };
    let preview = imports::preview(&store, &wrong_column, csv.as_bytes()).unwrap();
    assert!(preview.problem.unwrap().contains("Amount"));
    assert_eq!(preview.columns, ["Data", "Descrizione", "Importo"]);
}

#[test]
fn the_import_profile_is_saved_per_account_through_the_sync_log_and_can_be_edited() {
    let mut store = store();
    let account = checking(&mut store);
    assert_eq!(
        imports::profile_for_account(&store, &account.id).unwrap(),
        None
    );

    let saved = imports::save_profile(&mut store, italian_profile(&account)).unwrap();
    assert_eq!(
        imports::profile_for_account(&store, &account.id).unwrap(),
        Some(saved.clone())
    );

    let edited = imports::save_profile(
        &mut store,
        ImportProfile {
            skip_rows: 3,
            balance_column: Some("Saldo".into()),
            ..saved.clone()
        },
    )
    .unwrap();
    assert_eq!(edited.id, saved.id, "still the Account's one profile");
    assert_eq!(
        imports::profile_for_account(&store, &account.id).unwrap(),
        Some(edited.clone())
    );

    let ops = store.ops_since(store.device_id(), 0).unwrap();
    let last = ops.last().unwrap();
    assert_eq!(last.entity, "import_profile");
    assert_eq!(
        last.decode_payload::<ImportProfile>().unwrap(),
        Some(edited)
    );
}

#[test]
fn the_same_account_gets_the_same_profile_id_on_every_device() {
    let mut pc = store();
    let account = checking(&mut pc);
    let mut phone = store();
    phone.write(|w| w.upsert(&account)).unwrap();

    let on_pc = imports::save_profile(&mut pc, italian_profile(&account)).unwrap();
    let on_phone = imports::save_profile(&mut phone, italian_profile(&account)).unwrap();

    assert_eq!(on_pc.id, on_phone.id);
}

#[test]
fn a_profile_without_the_columns_it_needs_is_refused() {
    let mut store = store();
    let account = checking(&mut store);
    let no_debit = ImportProfile {
        amount_mode: AmountMode::Split,
        amount_column: None,
        debit_column: None,
        credit_column: Some("Avere".into()),
        ..italian_profile(&account)
    };
    let no_description = ImportProfile {
        description_columns: vec![],
        ..italian_profile(&account)
    };
    let bad_date_format = ImportProfile {
        date_format: "%d/%m".into(),
        ..italian_profile(&account)
    };

    for profile in [no_debit, no_description, bad_date_format] {
        assert!(matches!(
            imports::save_profile(&mut store, profile),
            Err(Error::Invalid(_))
        ));
    }
    assert_eq!(
        imports::profile_for_account(&store, &account.id).unwrap(),
        None
    );
}

const NOVEMBER: &str = "Data;Descrizione;Importo\n\
                        03/11/2026;Esselunga;-12,50\n\
                        05/11/2026;Farmacia;-8,00\n\
                        27/11/2026;Stipendio;1.250,00\n";

/// (date, amount, description) of the Account's Transactions, oldest first.
fn account_lines(store: &Store, account: &Account) -> Vec<(String, i64, String)> {
    let mut lines: Vec<_> = transactions::list_for_account(store, &account.id)
        .unwrap()
        .into_iter()
        .map(|v| {
            (
                v.transaction.date,
                v.transaction.amount_minor,
                v.transaction.description,
            )
        })
        .collect();
    lines.reverse();
    lines
}

fn line(date: &str, amount: i64, description: &str) -> (String, i64, String) {
    (date.into(), amount, description.into())
}

#[test]
fn an_import_creates_imported_transactions_and_appears_in_the_import_history() {
    let mut store = store();
    let account = checking(&mut store);
    imports::save_profile(&mut store, italian_profile(&account)).unwrap();

    let summary =
        imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();

    assert_eq!((summary.batch.rows_new, summary.batch.rows_skipped), (3, 0));
    assert_eq!(
        account_lines(&store, &account),
        [
            line("2026-11-03", -1_250, "Esselunga"),
            line("2026-11-05", -800, "Farmacia"),
            line("2026-11-27", 125_000, "Stipendio"),
        ]
    );
    let views = transactions::list_for_account(&store, &account.id).unwrap();
    assert!(views
        .iter()
        .all(|v| v.transaction.source == TxnSource::Import
            && v.transaction.import_id.as_deref() == Some(summary.batch.id.as_str())
            && v.transaction.fingerprint.is_some()));

    let history = imports::history(&store, &account.id).unwrap();
    assert_eq!(history, std::slice::from_ref(&summary.batch));
    assert_eq!(history[0].source_name, "nov.csv");
    assert_eq!(history[0].first_date.as_deref(), Some("2026-11-03"));
    assert_eq!(history[0].last_date.as_deref(), Some("2026-11-27"));
}

#[test]
fn importing_into_an_account_without_an_import_profile_is_refused() {
    let mut store = store();
    let account = checking(&mut store);

    let result = imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes());

    assert!(matches!(result, Err(Error::Invalid(_))));
}

/// A Store with an Account and its Italian-style Import profile.
fn store_with_profile() -> (Store, Account) {
    let mut store = store();
    let account = checking(&mut store);
    imports::save_profile(&mut store, italian_profile(&account)).unwrap();
    (store, account)
}

fn new_and_skipped(summary: &imports::ImportSummary) -> (i64, i64) {
    (summary.batch.rows_new, summary.batch.rows_skipped)
}

#[test]
fn importing_the_same_file_twice_skips_every_line_the_second_time() {
    let (mut store, account) = store_with_profile();
    imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();

    let again =
        imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();

    assert_eq!(new_and_skipped(&again), (0, 3));
    assert_eq!(account_lines(&store, &account).len(), 3);
    let history = imports::history(&store, &account.id).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, again.batch.id, "newest first");
}

#[test]
fn two_files_with_overlapping_dates_import_the_shared_lines_once() {
    let (mut store, account) = store_with_profile();
    let first_half = "Data;Descrizione;Importo\n\
                      03/11/2026;Esselunga;-12,50\n\
                      05/11/2026;Farmacia;-8,00\n";
    // Newest first, as many banks write it, overlapping on 5 November.
    let second_half = "Data;Descrizione;Importo\n\
                       27/11/2026;Stipendio;1.250,00\n\
                       05/11/2026;Farmacia;-8,00\n";

    imports::import_file(&mut store, &account.id, "a.csv", first_half.as_bytes()).unwrap();
    let second =
        imports::import_file(&mut store, &account.id, "b.csv", second_half.as_bytes()).unwrap();

    assert_eq!(new_and_skipped(&second), (1, 1));
    assert_eq!(
        account_lines(&store, &account),
        [
            line("2026-11-03", -1_250, "Esselunga"),
            line("2026-11-05", -800, "Farmacia"),
            line("2026-11-27", 125_000, "Stipendio"),
        ]
    );
}

#[test]
fn two_identical_lines_on_the_same_day_are_two_transactions_and_stay_two_on_reimport() {
    let (mut store, account) = store_with_profile();
    let two_coffees = "Data;Descrizione;Importo\n\
                       04/11/2026;Bar Centrale;-2,00\n\
                       04/11/2026;Bar Centrale;-2,00\n";
    let three_coffees = format!("{two_coffees}04/11/2026;BAR  centrale;-2,00\n");

    let first =
        imports::import_file(&mut store, &account.id, "a.csv", two_coffees.as_bytes()).unwrap();
    assert_eq!(new_and_skipped(&first), (2, 0));

    // A later file has a third identical coffee (description differs only in
    // case and spacing): only that one is new.
    let second =
        imports::import_file(&mut store, &account.id, "b.csv", three_coffees.as_bytes()).unwrap();
    assert_eq!(new_and_skipped(&second), (1, 2));
    assert_eq!(account_lines(&store, &account).len(), 3);
}

#[test]
fn the_same_file_imported_on_two_devices_gives_the_same_transaction_ids() {
    let (mut pc, account) = store_with_profile();
    let mut phone = store();
    phone.write(|w| w.upsert(&account)).unwrap();
    imports::save_profile(&mut phone, italian_profile(&account)).unwrap();

    imports::import_file(&mut pc, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();
    imports::import_file(&mut phone, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();

    let ids = |store: &Store| -> Vec<String> {
        let mut ids: Vec<_> = transactions::list_for_account(store, &account.id)
            .unwrap()
            .into_iter()
            .map(|v| v.transaction.id)
            .collect();
        ids.sort();
        ids
    };
    assert_eq!(ids(&pc).len(), 3);
    assert_eq!(ids(&pc), ids(&phone));
}

#[test]
fn a_deleted_imported_transaction_does_not_come_back_on_reimport() {
    let (mut store, account) = store_with_profile();
    imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();
    let pharmacy = transactions::list_for_account(&store, &account.id)
        .unwrap()
        .into_iter()
        .find(|v| v.transaction.description == "Farmacia")
        .unwrap();
    transactions::delete(&mut store, &pharmacy.transaction.id).unwrap();

    let again =
        imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();

    assert_eq!(new_and_skipped(&again), (0, 3));
    assert_eq!(account_lines(&store, &account).len(), 2);
}

#[test]
fn a_file_with_a_line_that_cant_be_read_imports_nothing() {
    let (mut store, account) = store_with_profile();
    let csv = "Data;Descrizione;Importo\n\
               03/11/2026;Esselunga;-12,50\n\
               05/11/2026;Farmacia;otto euro\n";

    let result = imports::import_file(&mut store, &account.id, "bad.csv", csv.as_bytes());

    let Err(Error::Invalid(message)) = result else {
        panic!("expected the Import to be refused");
    };
    assert!(message.contains("Line 3"), "{message}");
    assert!(account_lines(&store, &account).is_empty());
    assert!(imports::history(&store, &account.id).unwrap().is_empty());
}

fn with_balance(account: &Account) -> ImportProfile {
    ImportProfile {
        balance_column: Some("Saldo".into()),
        ..italian_profile(account)
    }
}

#[test]
fn no_warning_when_the_balance_column_agrees_with_the_calculated_balance() {
    let mut store = store();
    let account = checking(&mut store); // 1,000.00 on 2026-10-31
    imports::save_profile(&mut store, with_balance(&account)).unwrap();
    // Newest first; two lines on 3 November, the bank's balance after each.
    let csv = "Data;Descrizione;Importo;Saldo\n\
               05/11/2026;Farmacia;-8,00;979,50\n\
               03/11/2026;Bar;-2,50;987,50\n\
               03/11/2026;Esselunga;-10,00;990,00\n";

    let summary = imports::import_file(&mut store, &account.id, "nov.csv", csv.as_bytes()).unwrap();

    assert_eq!(summary.balance_mismatch, None);
}

#[test]
fn the_first_line_whose_balance_disagrees_with_the_calculated_balance_is_reported() {
    let mut store = store();
    let account = checking(&mut store); // 1,000.00 on 2026-10-31
    imports::save_profile(&mut store, with_balance(&account)).unwrap();
    // The bank's balance is off by 1.00 from 5 November on.
    let csv = "Data;Descrizione;Importo;Saldo\n\
               03/11/2026;Esselunga;-12,50;987,50\n\
               05/11/2026;Farmacia;-8,00;978,50\n\
               27/11/2026;Stipendio;1.250,00;2.228,50\n";

    let summary = imports::import_file(&mut store, &account.id, "nov.csv", csv.as_bytes()).unwrap();

    let mismatch = summary.balance_mismatch.expect("a warning");
    assert_eq!(mismatch.line, 3);
    assert_eq!(mismatch.date, "2026-11-05");
    assert_eq!(mismatch.description, "Farmacia");
    assert_eq!(mismatch.file_balance_minor, 97_850);
    assert_eq!(mismatch.calculated_balance_minor, 97_950);
}

#[test]
fn without_a_balance_column_there_is_no_balance_check() {
    let (mut store, account) = store_with_profile();
    // A Manual Transaction the bank doesn't know about would not matter anyway.
    let summary =
        imports::import_file(&mut store, &account.id, "nov.csv", NOVEMBER.as_bytes()).unwrap();
    assert_eq!(summary.balance_mismatch, None);
}
