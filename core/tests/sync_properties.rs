//! Property-style tests of Sync: random independent edits on several
//! Devices, checked against the guarantees in sync-design.md §1.4 and §3.2.
//! Each case is reproducible from its seed.

mod common;

use budget_core::accounts::{Account, AccountKind};
use budget_core::settings::Setting;
use budget_core::store::Op;
use budget_core::sync::registry::SYNCED_ENTITIES;
use budget_core::Store;
use common::{all_ops, sync_pair, synced_rows, Rng};

const SEEDS: u64 = 40;
const ROW_IDS: [&str; 5] = ["acc-1", "acc-2", "acc-3", "acc-4", "acc-5"];

/// Make `count` random edits: create or edit an Account (possibly one that
/// another Device has deleted), delete one, or change a synced setting.
/// The wall clock jumps around, backwards too, so clock ties and skew occur.
pub fn random_edits(store: &mut Store, rng: &mut Rng, count: usize) {
    for _ in 0..count {
        let now = 1_000_000 + rng.below(50) as u64;
        store.set_wall_clock(move || now);
        let id = ROW_IDS[rng.below(ROW_IDS.len())];
        let live: bool = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM account WHERE id = ?1 AND deleted = 0",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        match rng.below(4) {
            0 if live => {
                store.write(|w| w.delete::<Account>(id)).unwrap();
            }
            1 => {
                let value = format!("\"{}\"", ["EUR", "USD", "GBP"][rng.below(3)]);
                store
                    .write(|w| {
                        w.upsert(&Setting {
                            key: "currency".into(),
                            value,
                        })
                    })
                    .unwrap();
            }
            _ => {
                let row = Account {
                    id: id.into(),
                    name: format!("Account {}", rng.below(1000)),
                    kind: AccountKind::Savings,
                    opening_minor: rng.below(100_000) as i64 - 50_000,
                    opening_date: "2026-01-01".into(),
                    is_main: false,
                };
                store.write(|w| w.upsert(&row)).unwrap();
            }
        }
    }
}

#[test]
fn the_same_set_of_changes_applied_in_different_orders_gives_the_same_result() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed);
        // Three Devices edit the same rows; each sees some of the others' work
        // first, so later edits overwrite and delete rows made elsewhere.
        let mut devices: Vec<Store> = (0..3).map(|_| Store::open_in_memory().unwrap()).collect();
        for round in 0..3 {
            for d in 0..devices.len() {
                random_edits(&mut devices[d], &mut rng, 4);
                let ops = all_ops(&devices[d]);
                let next = (d + round + 1) % devices.len();
                devices[next].apply_remote(&ops).unwrap();
            }
        }
        let mut changes: Vec<Op> = devices.iter().flat_map(all_ops).collect();
        changes.sort_by(|a, b| (&a.origin, a.seq).cmp(&(&b.origin, b.seq)));
        changes.dedup();

        let mut results = Vec::new();
        for _ in 0..4 {
            rng.shuffle(&mut changes);
            let mut store = Store::open_in_memory().unwrap();
            // Random batch sizes, so some changes arrive alone and some together.
            let mut rest = &changes[..];
            while !rest.is_empty() {
                let (batch, tail) = rest.split_at(1 + rng.below(rest.len().min(6)));
                store.apply_remote(batch).unwrap();
                rest = tail;
            }
            results.push((synced_rows(&store), all_ops(&store).len()));
        }

        for result in &results[1..] {
            assert_eq!(result, &results[0], "seed {seed}");
        }
        assert_eq!(results[0].1, changes.len(), "seed {seed}");
    }
}

#[test]
fn two_devices_with_random_independent_edits_end_up_identical_after_a_sync() {
    for seed in 0..SEEDS {
        let mut rng = Rng::new(seed);
        let mut pc = Store::open_in_memory().unwrap();
        let mut phone = Store::open_in_memory().unwrap();
        // Some shared history first, then offline edits on both, more than once.
        random_edits(&mut pc, &mut rng, 5);
        let (first, _) = sync_pair(&mut pc, &mut phone);
        first.unwrap();
        for _ in 0..3 {
            let n = 1 + rng.below(8);
            random_edits(&mut pc, &mut rng, n);
            let n = 1 + rng.below(8);
            random_edits(&mut phone, &mut rng, n);

            let (on_pc, on_phone) = sync_pair(&mut pc, &mut phone);

            assert!(on_pc.unwrap().converged, "seed {seed}");
            assert!(on_phone.unwrap().converged, "seed {seed}");
            assert_eq!(synced_rows(&pc), synced_rows(&phone), "seed {seed}");
            assert_eq!(all_ops(&pc), all_ops(&phone), "seed {seed}");
        }
    }
}

#[test]
fn every_synced_table_is_registered_for_sync() {
    let store = Store::open_in_memory().unwrap();
    let mut synced_tables: Vec<String> = store
        .conn()
        .prepare(
            "SELECT m.name FROM sqlite_master m, pragma_table_info(m.name) c
             WHERE m.type = 'table' AND c.name = 'updated_hlc' ORDER BY m.name",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut registered: Vec<String> = SYNCED_ENTITIES.iter().map(|e| e.name.to_owned()).collect();
    synced_tables.sort();
    registered.sort();

    assert_eq!(
        registered, synced_tables,
        "add every SyncedRow to SYNCED_ENTITIES in core/src/sync/registry.rs"
    );
}
