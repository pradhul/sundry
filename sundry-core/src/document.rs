use crate::category_filter::item_fits_category;
use crate::llm::{normalise, GeneratedItem};
use crate::model::{Category, Item, ItemSource, ItemState, Owner, Trip};
use automerge::transaction::Transactable;
use automerge::{AutoCommit, ObjType, ReadDoc, ROOT};
use std::collections::HashSet;
use uuid::Uuid;

const TRIP_KEY: &str = "trip";
const ITEMS_KEY: &str = "items";

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("automerge error: {0}")]
    Automerge(String),
    #[error("document is malformed: {0}")]
    Malformed(String),
    #[error("no item with id {0}")]
    NoSuchItem(Uuid),
    #[error("no traveller named {0}")]
    NoSuchTraveller(String),
}

impl From<automerge::AutomergeError> for DocError {
    fn from(e: automerge::AutomergeError) -> Self {
        DocError::Automerge(e.to_string())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub added: usize,
    pub skipped_existing: usize,
    pub skipped_off_category: usize,
}

pub struct TripDocument {
    doc: AutoCommit,
}

impl TripDocument {
    pub fn new(trip: &Trip) -> Self {
        let mut doc = AutoCommit::new();
        let json = serde_json::to_string(trip).expect("Trip serialises");
        doc.put(ROOT, TRIP_KEY, json).expect("put trip");
        doc.put_object(ROOT, ITEMS_KEY, ObjType::Map)
            .expect("put items map");
        TripDocument { doc }
    }

    pub fn load(bytes: &[u8]) -> Result<Self, DocError> {
        let doc = AutoCommit::load(bytes)?;
        Ok(TripDocument { doc })
    }

    pub fn save(&mut self) -> Vec<u8> {
        self.doc.save()
    }

    pub fn merge(&mut self, other: &mut TripDocument) -> Result<(), DocError> {
        self.doc.merge(&mut other.doc)?;
        Ok(())
    }

    pub fn trip(&self) -> Result<Trip, DocError> {
        let raw = self.string_at(&ROOT, TRIP_KEY)?;
        serde_json::from_str(&raw).map_err(|e| DocError::Malformed(e.to_string()))
    }

    fn put_trip(&mut self, trip: &Trip) -> Result<(), DocError> {
        let json = serde_json::to_string(trip).map_err(|e| DocError::Malformed(e.to_string()))?;
        self.doc.put(ROOT, TRIP_KEY, json)?;
        Ok(())
    }

    pub fn items(&self) -> Result<Vec<Item>, DocError> {
        let items_obj = self.items_obj()?;
        let mut out = Vec::new();
        for key in self.doc.keys(&items_obj) {
            let raw = self.string_at(&items_obj, &key)?;
            let item: Item =
                serde_json::from_str(&raw).map_err(|e| DocError::Malformed(e.to_string()))?;
            out.push(item);
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn add_item(&mut self, item: &Item) -> Result<(), DocError> {
        let items_obj = self.items_obj()?;
        let json = serde_json::to_string(item).map_err(|e| DocError::Malformed(e.to_string()))?;
        self.doc.put(&items_obj, item.id.to_string(), json)?;
        Ok(())
    }

    pub fn remove_item(&mut self, id: Uuid) -> Result<(), DocError> {
        let items_obj = self.items_obj()?;
        let key = id.to_string();
        if self.doc.get(&items_obj, &key)?.is_none() {
            return Err(DocError::NoSuchItem(id));
        }
        self.doc.delete(&items_obj, key)?;
        Ok(())
    }

    pub fn set_item_state(&mut self, id: Uuid, state: ItemState) -> Result<(), DocError> {
        let mut item = self
            .items()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or(DocError::NoSuchItem(id))?;
        item.state = state;
        if matches!(state, ItemState::Todo | ItemState::NotNeeded) {
            item.owner = Owner::Shared;
        }
        self.add_item(&item)
    }

    pub fn set_item_packed_for(&mut self, id: Uuid, traveller_name: &str) -> Result<(), DocError> {
        let trip = self.trip()?;
        let traveller = trip
            .travellers
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(traveller_name))
            .ok_or_else(|| DocError::NoSuchTraveller(traveller_name.to_string()))?;
        let traveller_id = traveller.id;

        let mut item = self
            .items()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or(DocError::NoSuchItem(id))?;
        item.state = ItemState::Packed;
        item.owner = Owner::Traveller(traveller_id);
        self.add_item(&item)
    }

    /// Rename/replace travellers by order so the UI constants (Appu, Swathi) stick.
    pub fn ensure_travellers(&mut self, names: &[String]) -> Result<(), DocError> {
        let mut trip = self.trip()?;
        let same = trip
            .travellers
            .iter()
            .map(|t| t.name.as_str())
            .eq(names.iter().map(|s| s.as_str()));
        if same {
            return Ok(());
        }

        let old_ids: Vec<Uuid> = trip.travellers.iter().map(|t| t.id).collect();
        let mut new_travellers = Vec::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            let id = old_ids.get(i).copied().unwrap_or_else(Uuid::new_v4);
            new_travellers.push(crate::model::Traveller {
                id,
                name: name.clone(),
            });
        }
        let kept: HashSet<Uuid> = new_travellers.iter().map(|t| t.id).collect();
        trip.travellers = new_travellers;
        self.put_trip(&trip)?;

        for mut item in self.items()? {
            if let Owner::Traveller(id) = item.owner {
                if !kept.contains(&id) {
                    item.owner = Owner::Shared;
                    if item.state == ItemState::Packed {
                        item.state = ItemState::Todo;
                    }
                    self.add_item(&item)?;
                }
            } else if item.state == ItemState::Packed {
                // Legacy packed rows with no traveller — send back to to-pack.
                item.state = ItemState::Todo;
                self.add_item(&item)?;
            }
        }
        Ok(())
    }

    /// Keep one item per normalised name. Prefer essential, earlier category, then non-todo.
    pub fn dedupe_by_normalised_name(&mut self) -> Result<usize, DocError> {
        let items = self.items()?;
        let mut best: std::collections::HashMap<String, Item> = std::collections::HashMap::new();
        let mut losers: Vec<Uuid> = Vec::new();

        for item in items {
            let key = normalise(&item.name);
            match best.get(&key) {
                None => {
                    best.insert(key, item);
                }
                Some(winner) => {
                    if Self::is_better_duplicate(&item, winner) {
                        losers.push(winner.id);
                        best.insert(key, item);
                    } else {
                        losers.push(item.id);
                    }
                }
            }
        }

        let removed = losers.len();
        for id in losers {
            self.remove_item(id)?;
        }
        Ok(removed)
    }

    fn is_better_duplicate(candidate: &Item, incumbent: &Item) -> bool {
        match (candidate.essential, incumbent.essential) {
            (true, false) => return true,
            (false, true) => return false,
            _ => {}
        }
        let cand_cat = category_rank(candidate.category);
        let inc_cat = category_rank(incumbent.category);
        if cand_cat != inc_cat {
            return cand_cat < inc_cat;
        }
        state_rank(candidate.state) > state_rank(incumbent.state)
    }

    pub fn merge_generated(
        &mut self,
        category: Category,
        generated: &[GeneratedItem],
    ) -> Result<MergeReport, DocError> {
        let mut existing: Vec<Item> = self.items()?;

        let mut report = MergeReport::default();

        for g in generated {
            let Some(name) = crate::item_quality::clean_name(category, &g.name) else {
                report.skipped_off_category += 1;
                continue;
            };
            if !item_fits_category(category, &name) {
                report.skipped_off_category += 1;
                continue;
            }
            // Same-category variants ("Sweater" vs "Thermal sweater"): keep the
            // more specific name on a single row.
            if let Some(pos) = existing.iter().position(|e| {
                e.category == category
                    && (crate::item_quality::refines(&e.name, &name)
                        || crate::item_quality::refines(&name, &e.name))
            }) {
                let incumbent = &mut existing[pos];
                if crate::item_quality::refines(&incumbent.name, &name)
                    && incumbent.source == ItemSource::Model
                    && incumbent.state == ItemState::Todo
                {
                    incumbent.name = name.clone();
                    incumbent.quantity = incumbent.quantity.max(g.quantity);
                    let updated = incumbent.clone();
                    self.add_item(&updated)?;
                }
                report.skipped_existing += 1;
                continue;
            }
            let key = normalise(&name);
            if existing.iter().any(|e| {
                normalise(&e.name) == key || crate::item_quality::is_near_duplicate(&e.name, &name)
            }) {
                report.skipped_existing += 1;
                continue;
            }
            let item = Item {
                id: Uuid::new_v4(),
                name,
                quantity: g.quantity,
                category,
                owner: Owner::Shared,
                state: ItemState::Todo,
                source: ItemSource::Model,
                rationale: Some(g.rationale.clone()),
                note: None,
                essential: g.essential,
            };
            self.add_item(&item)?;
            existing.push(item);
            report.added += 1;
        }

        Ok(report)
    }

    fn items_obj(&self) -> Result<automerge::ObjId, DocError> {
        self.doc
            .get(ROOT, ITEMS_KEY)?
            .map(|(_, id)| id)
            .ok_or_else(|| DocError::Malformed("missing items map".into()))
    }

    fn string_at<O: AsRef<automerge::ObjId>>(&self, obj: O, key: &str) -> Result<String, DocError> {
        match self.doc.get(obj, key)? {
            Some((value, _)) => value
                .into_string()
                .map_err(|_| DocError::Malformed(format!("{key} is not a string"))),
            None => Err(DocError::Malformed(format!("missing key {key}"))),
        }
    }
}

fn category_rank(category: Category) -> usize {
    Category::ALL
        .iter()
        .position(|c| *c == category)
        .unwrap_or(usize::MAX)
}

fn state_rank(state: ItemState) -> u8 {
    match state {
        ItemState::Todo => 0,
        ItemState::NotNeeded => 1,
        ItemState::Packed => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

    fn a_trip() -> Trip {
        Trip {
            id: Uuid::new_v4(),
            title: "Kyoto".into(),
            destinations: vec![Destination {
                country_code: "JP".into(),
                city: "Kyoto".into(),
            }],
            window: TravelWindow::Month {
                year: 2026,
                month: 11,
            },
            nights: 7,
            travellers: vec![
                Traveller {
                    id: Uuid::new_v4(),
                    name: "Appu".into(),
                },
                Traveller {
                    id: Uuid::new_v4(),
                    name: "Swathi".into(),
                },
            ],
            purposes: vec![Purpose::City],
        }
    }

    fn generated(name: &str) -> GeneratedItem {
        GeneratedItem {
            name: name.into(),
            quantity: 1,
            rationale: "because".into(),
            essential: false,
        }
    }

    #[test]
    fn round_trips_through_bytes() {
        let trip = a_trip();
        let mut doc = TripDocument::new(&trip);
        let bytes = doc.save();
        let reloaded = TripDocument::load(&bytes).unwrap();
        assert_eq!(reloaded.trip().unwrap(), trip);
    }

    #[test]
    fn stores_and_reads_items() {
        let mut doc = TripDocument::new(&a_trip());
        let item = Item::manual("Passport", Category::Documents);
        doc.add_item(&item).unwrap();
        let items = doc.items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Passport");
    }

    #[test]
    fn updates_item_state() {
        let mut doc = TripDocument::new(&a_trip());
        let item = Item::manual("Passport", Category::Documents);
        doc.add_item(&item).unwrap();
        doc.set_item_state(item.id, ItemState::Packed).unwrap();
        assert_eq!(doc.items().unwrap()[0].state, ItemState::Packed);
    }

    #[test]
    fn unpacking_clears_owner() {
        let mut doc = TripDocument::new(&a_trip());
        let item = Item::manual("Passport", Category::Documents);
        doc.add_item(&item).unwrap();
        doc.set_item_packed_for(item.id, "Appu").unwrap();
        assert!(matches!(doc.items().unwrap()[0].owner, Owner::Traveller(_)));
        doc.set_item_state(item.id, ItemState::Todo).unwrap();
        assert_eq!(doc.items().unwrap()[0].owner, Owner::Shared);
        assert_eq!(doc.items().unwrap()[0].state, ItemState::Todo);
    }

    #[test]
    fn packs_for_named_traveller() {
        let mut doc = TripDocument::new(&a_trip());
        let item = Item::manual("Passport", Category::Documents);
        doc.add_item(&item).unwrap();
        doc.set_item_packed_for(item.id, "Swathi").unwrap();
        let packed = &doc.items().unwrap()[0];
        assert_eq!(packed.state, ItemState::Packed);
        let trip = doc.trip().unwrap();
        let swathi = trip.travellers.iter().find(|t| t.name == "Swathi").unwrap();
        assert_eq!(packed.owner, Owner::Traveller(swathi.id));
    }

    #[test]
    fn merge_adds_new_items_as_todo_from_the_model() {
        let mut doc = TripDocument::new(&a_trip());
        let report = doc
            .merge_generated(Category::Electronics, &[generated("USB-C charger")])
            .unwrap();
        assert_eq!(report.added, 1);
        let items = doc.items().unwrap();
        assert_eq!(items[0].state, ItemState::Todo);
        assert_eq!(items[0].source, ItemSource::Model);
        assert_eq!(items[0].rationale.as_deref(), Some("because"));
    }

    #[test]
    fn merge_skips_off_category_items() {
        let mut doc = TripDocument::new(&a_trip());
        let report = doc
            .merge_generated(
                Category::Money,
                &[generated("Comfortable walking shoes"), generated("Wallet")],
            )
            .unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped_off_category, 1);
        assert_eq!(doc.items().unwrap()[0].name, "Wallet");
    }

    #[test]
    fn regeneration_never_mutates_existing_state() {
        let mut doc = TripDocument::new(&a_trip());
        doc.merge_generated(Category::Electronics, &[generated("USB-C charger")])
            .unwrap();
        let id = doc.items().unwrap()[0].id;
        doc.set_item_state(id, ItemState::Packed).unwrap();

        let report = doc
            .merge_generated(Category::Electronics, &[generated("USB-C charger")])
            .unwrap();

        assert_eq!(report.added, 0);
        assert_eq!(report.skipped_existing, 1);
        let items = doc.items().unwrap();
        assert_eq!(items.len(), 1, "must not duplicate the item");
        assert_eq!(
            items[0].state,
            ItemState::Packed,
            "must not reset packed state"
        );
    }

    #[test]
    fn regeneration_preserves_manual_items_the_model_did_not_suggest() {
        let mut doc = TripDocument::new(&a_trip());
        let mine = Item::manual("Lucky socks", Category::Clothing);
        doc.add_item(&mine).unwrap();
        doc.merge_generated(Category::Clothing, &[generated("Rain jacket")])
            .unwrap();
        let names: Vec<String> = doc.items().unwrap().into_iter().map(|i| i.name).collect();
        assert!(names.contains(&"Lucky socks".to_string()));
        assert!(names.contains(&"Rain jacket".to_string()));
    }

    #[test]
    fn regeneration_preserves_not_needed_state() {
        let mut doc = TripDocument::new(&a_trip());
        doc.merge_generated(Category::Clothing, &[generated("Umbrella")])
            .unwrap();
        let id = doc.items().unwrap()[0].id;
        doc.set_item_state(id, ItemState::NotNeeded).unwrap();
        doc.merge_generated(Category::Clothing, &[generated("Umbrella")])
            .unwrap();
        assert_eq!(doc.items().unwrap()[0].state, ItemState::NotNeeded);
    }

    #[test]
    fn matching_is_case_and_whitespace_insensitive() {
        let mut doc = TripDocument::new(&a_trip());
        doc.merge_generated(Category::Clothing, &[generated("Rain  Jacket")])
            .unwrap();
        let report = doc
            .merge_generated(Category::Clothing, &[generated("rain jacket")])
            .unwrap();
        assert_eq!(report.added, 0);
    }

    #[test]
    fn same_name_in_a_different_category_is_skipped() {
        let mut doc = TripDocument::new(&a_trip());
        doc.merge_generated(Category::Clothing, &[generated("Travel scarf")])
            .unwrap();
        let report = doc
            .merge_generated(Category::Misc, &[generated("Travel scarf")])
            .unwrap();
        assert_eq!(report.added, 0);
        assert_eq!(report.skipped_existing, 1);
        assert_eq!(doc.items().unwrap().len(), 1);
        assert_eq!(doc.items().unwrap()[0].category, Category::Clothing);
    }

    #[test]
    fn dedupe_keeps_essential_winner() {
        let mut doc = TripDocument::new(&a_trip());
        let mut non = Item::manual("Sunscreen", Category::Documents);
        non.essential = false;
        doc.add_item(&non).unwrap();
        let mut ess = Item::manual("Sunscreen", Category::Toiletries);
        ess.essential = true;
        doc.add_item(&ess).unwrap();

        let removed = doc.dedupe_by_normalised_name().unwrap();
        assert_eq!(removed, 1);
        let survivors: Vec<_> = doc
            .items()
            .unwrap()
            .into_iter()
            .filter(|i| normalise(&i.name) == "sunscreen")
            .collect();
        assert_eq!(survivors.len(), 1);
        assert!(survivors[0].essential);
        assert_eq!(survivors[0].category, Category::Toiletries);
    }

    #[test]
    fn ensure_travellers_renames_in_place() {
        let mut doc = TripDocument::new(&a_trip());
        doc.ensure_travellers(&["Appu".into(), "Swathi".into()])
            .unwrap();
        let names: Vec<_> = doc
            .trip()
            .unwrap()
            .travellers
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["Appu".to_string(), "Swathi".to_string()]);
    }

    #[test]
    fn two_documents_merge_without_conflict() {
        let trip = a_trip();
        let mut a = TripDocument::new(&trip);
        a.merge_generated(Category::Clothing, &[generated("Rain jacket")])
            .unwrap();
        let mut b = TripDocument::load(&a.save()).unwrap();

        a.add_item(&Item::manual("From A", Category::Misc)).unwrap();
        b.add_item(&Item::manual("From B", Category::Misc)).unwrap();

        a.merge(&mut b).unwrap();
        let names: Vec<String> = a.items().unwrap().into_iter().map(|i| i.name).collect();
        assert!(names.contains(&"From A".to_string()));
        assert!(names.contains(&"From B".to_string()));
    }

    #[test]
    fn variant_merges_into_the_more_specific_name() {
        let mut doc = TripDocument::new(&a_trip());
        doc.merge_generated(Category::Clothing, &[generated("Sweater")])
            .unwrap();
        doc.merge_generated(Category::Clothing, &[generated("Thermal sweater")])
            .unwrap();
        doc.merge_generated(
            Category::Clothing,
            &[generated("Rain jacket"), generated("Down jacket")],
        )
        .unwrap();
        let names: Vec<String> = doc.items().unwrap().into_iter().map(|i| i.name).collect();
        assert!(names.contains(&"Thermal sweater".to_string()), "{names:?}");
        assert!(!names.contains(&"Sweater".to_string()));
        assert!(names.contains(&"Rain jacket".to_string()));
        assert!(names.contains(&"Down jacket".to_string()));
        assert_eq!(names.len(), 3);
    }
}
