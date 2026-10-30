//! Read-only check of a COPY of a real Sundry store: every trip loads and
//! reports its stored travellers and per-traveller packed counts.
//!   cargo run -p sundry-core --example migration_check -- <copy-of-store>
use std::path::PathBuf;
use sundry_core::model::Owner;
use sundry_core::store::Store;
fn main() {
    let store = Store::new(PathBuf::from(std::env::args().nth(1).expect("store path")));
    for e in store.list().unwrap() {
        let doc = store.load(e.id).unwrap();
        let trip = doc.trip().unwrap();
        let items = doc.items().unwrap();
        let packed: Vec<String> = trip
            .travellers
            .iter()
            .map(|t| {
                let n = items
                    .iter()
                    .filter(|i| i.owner == Owner::Traveller(t.id))
                    .count();
                format!("{}={}", t.name, n)
            })
            .collect();
        println!(
            "{} {} items={} travellers=[{}]",
            e.id,
            e.title,
            items.len(),
            packed.join(", ")
        );
    }
}
