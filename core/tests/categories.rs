//! Categories: two levels, each income or expense (CONTEXT.md).

use budget_core::categories::{self, CategoryKind, NewCategory};
use budget_core::store::OpKind;
use budget_core::{Error, Store};

fn top(name: &str, kind: CategoryKind) -> NewCategory {
    NewCategory {
        name: name.into(),
        kind,
        parent_id: None,
    }
}

fn names(store: &Store) -> Vec<String> {
    categories::list(store)
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect()
}

#[test]
fn a_created_category_is_listed_and_its_creation_is_logged() {
    let mut store = Store::open_in_memory().unwrap();

    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();

    let listed = categories::list(&store).unwrap();
    assert_eq!(listed, vec![food.clone()]);
    assert_eq!(food.kind, CategoryKind::Expense);
    assert_eq!(food.parent_id, None);
    let ops = store.ops_since(store.device_id(), 0).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(
        (
            ops[0].entity.as_str(),
            ops[0].entity_id.as_str(),
            ops[0].kind
        ),
        ("category", food.id.as_str(), OpKind::Upsert)
    );
}

fn child(name: &str, parent: &categories::Category) -> NewCategory {
    NewCategory {
        name: name.into(),
        kind: parent.kind,
        parent_id: Some(parent.id.clone()),
    }
}

#[test]
fn each_parent_is_listed_followed_by_its_children_in_creation_order() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();
    let salary = categories::create(&mut store, top("Salary", CategoryKind::Income)).unwrap();
    categories::create(&mut store, child("Groceries", &food)).unwrap();
    categories::create(&mut store, child("Bonus", &salary)).unwrap();
    categories::create(&mut store, child("Restaurants", &food)).unwrap();

    assert_eq!(
        names(&store),
        ["Food", "Groceries", "Restaurants", "Salary", "Bonus"]
    );
}

#[test]
fn categories_have_at_most_two_levels() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();
    let groceries = categories::create(&mut store, child("Groceries", &food)).unwrap();

    let result = categories::create(&mut store, child("Fruit", &groceries));

    assert!(matches!(result, Err(Error::Invalid(_))));
    assert_eq!(names(&store), ["Food", "Groceries"]);
}

#[test]
fn a_child_has_the_same_kind_as_its_parent() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();

    let result = categories::create(
        &mut store,
        NewCategory {
            kind: CategoryKind::Income,
            ..child("Groceries", &food)
        },
    );

    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[test]
fn a_category_needs_a_name_and_an_existing_parent() {
    let mut store = Store::open_in_memory().unwrap();
    for bad in [
        top("  ", CategoryKind::Expense),
        NewCategory {
            parent_id: Some("no-such-category".into()),
            ..top("Groceries", CategoryKind::Expense)
        },
    ] {
        assert!(matches!(
            categories::create(&mut store, bad),
            Err(Error::Invalid(_))
        ));
    }
    assert!(categories::list(&store).unwrap().is_empty());
    assert!(store.ops_since(store.device_id(), 0).unwrap().is_empty());
}

#[test]
fn renaming_a_category_keeps_its_place_and_is_logged() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();
    categories::create(&mut store, top("Transport", CategoryKind::Expense)).unwrap();

    let renamed = categories::rename(&mut store, &food.id, " Food & drink ").unwrap();

    assert_eq!(renamed.name, "Food & drink");
    assert_eq!(names(&store), ["Food & drink", "Transport"]);
    let last = store.ops_since(store.device_id(), 2).unwrap();
    assert_eq!(last.len(), 1);
    assert_eq!(
        last[0]
            .decode_payload::<categories::Category>()
            .unwrap()
            .unwrap()
            .name,
        "Food & drink"
    );
    assert!(matches!(
        categories::rename(&mut store, &food.id, ""),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        categories::rename(&mut store, "missing", "x"),
        Err(Error::NotFound { .. })
    ));
}

#[test]
fn moving_a_category_among_its_siblings_writes_only_that_category() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();
    categories::create(&mut store, top("Home", CategoryKind::Expense)).unwrap();
    let transport =
        categories::create(&mut store, top("Transport", CategoryKind::Expense)).unwrap();
    categories::create(&mut store, child("Groceries", &food)).unwrap();
    let restaurants = categories::create(&mut store, child("Restaurants", &food)).unwrap();
    let before = store.ops_since(store.device_id(), 0).unwrap().len() as u64;

    categories::move_to(&mut store, &transport.id, 0).unwrap();
    assert_eq!(
        names(&store),
        ["Transport", "Food", "Groceries", "Restaurants", "Home"]
    );
    categories::move_to(&mut store, &transport.id, 1).unwrap();
    assert_eq!(
        names(&store),
        ["Food", "Groceries", "Restaurants", "Transport", "Home"]
    );
    categories::move_to(&mut store, &transport.id, 99).unwrap();
    assert_eq!(
        names(&store),
        ["Food", "Groceries", "Restaurants", "Home", "Transport"]
    );
    categories::move_to(&mut store, &restaurants.id, 0).unwrap();
    assert_eq!(
        names(&store),
        ["Food", "Restaurants", "Groceries", "Home", "Transport"]
    );

    let moves = store.ops_since(store.device_id(), before).unwrap();
    let touched: Vec<_> = moves.iter().map(|o| o.entity_id.clone()).collect();
    assert_eq!(
        touched,
        [&transport.id, &transport.id, &transport.id, &restaurants.id].map(String::clone)
    );
}

#[test]
fn deleting_a_parent_deletes_its_children_in_the_same_change() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();
    let groceries = categories::create(&mut store, child("Groceries", &food)).unwrap();
    categories::create(&mut store, top("Transport", CategoryKind::Expense)).unwrap();

    categories::delete(&mut store, &food.id).unwrap();

    assert_eq!(names(&store), ["Transport"]);
    let mut deleted: Vec<_> = store
        .ops_since(store.device_id(), 3)
        .unwrap()
        .into_iter()
        .map(|o| (o.kind, o.entity_id))
        .collect();
    deleted.sort_by(|a, b| a.1.cmp(&b.1));
    let mut expected = vec![(OpKind::Delete, food.id), (OpKind::Delete, groceries.id)];
    expected.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(deleted, expected);
}

#[test]
fn deleting_a_child_keeps_its_parent() {
    let mut store = Store::open_in_memory().unwrap();
    let food = categories::create(&mut store, top("Food", CategoryKind::Expense)).unwrap();
    let groceries = categories::create(&mut store, child("Groceries", &food)).unwrap();

    categories::delete(&mut store, &groceries.id).unwrap();

    assert_eq!(names(&store), ["Food"]);
    assert!(matches!(
        categories::delete(&mut store, &groceries.id),
        Err(Error::NotFound { .. })
    ));
}
