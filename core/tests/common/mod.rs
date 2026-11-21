//! Helpers shared by the sync tests.
#![allow(dead_code)]

use budget_core::store::Op;
use budget_core::sync::registry::SYNCED_ENTITIES;
use budget_core::sync::{self, memory, SyncReport};
use budget_core::{Result, Store};
use rusqlite::types::ValueRef;

/// Run one Sync session between `a` and `b`, each side in its own thread.
pub fn sync_pair(a: &mut Store, b: &mut Store) -> (Result<SyncReport>, Result<SyncReport>) {
    let (mut end_a, mut end_b) = memory::pair();
    std::thread::scope(|s| {
        let a = s.spawn(move || sync::run(a, &mut end_a));
        let b = s.spawn(move || sync::run(b, &mut end_b));
        (a.join().unwrap(), b.join().unwrap())
    })
}

/// Every op in the Device's sync log, ordered by (origin, seq).
pub fn all_ops(store: &Store) -> Vec<Op> {
    store
        .version_vector()
        .unwrap()
        .keys()
        .flat_map(|origin| store.ops_since(origin, 0).unwrap())
        .collect()
}

/// Every row (tombstones included, with `updated_hlc` and `deleted`) of
/// every synced table, as text, so two Devices can be compared.
pub fn synced_rows(store: &Store) -> Vec<String> {
    let mut out = Vec::new();
    for entity in SYNCED_ENTITIES {
        let mut stmt = store
            .conn()
            .prepare(&format!(
                "SELECT * FROM {} ORDER BY {}",
                entity.name, entity.id_column
            ))
            .unwrap();
        let columns = stmt.column_count();
        let rows = stmt
            .query_map([], |r| {
                let values: Vec<String> = (0..columns)
                    .map(|i| match r.get_ref(i).unwrap() {
                        ValueRef::Text(text) => format!("{:?}", String::from_utf8_lossy(text)),
                        other => format!("{other:?}"),
                    })
                    .collect();
                Ok(format!("{}: {}", entity.name, values.join(", ")))
            })
            .unwrap();
        out.extend(rows.map(Result::unwrap));
    }
    out
}

/// A small deterministic random generator (xorshift64*), so the
/// property-style tests are reproducible from their seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}
