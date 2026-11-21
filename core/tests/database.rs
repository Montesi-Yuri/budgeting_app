use budget_core::{Error, Store, SCHEMA_VERSION};

#[test]
fn a_new_database_is_migrated_to_the_current_schema() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
}

#[test]
fn a_database_written_by_a_newer_app_version_is_refused() {
    let dir = std::env::temp_dir().join(format!("budget-core-schema-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("budget.db");
    {
        let store = Store::open(&path).unwrap();
        store
            .conn()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
    }

    let result = Store::open(&path);

    assert!(matches!(result, Err(Error::SchemaTooNew { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}
