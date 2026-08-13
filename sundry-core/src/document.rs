use crate::llm::{normalise, GeneratedItem};
use crate::model::{Category, Item, ItemSource, ItemState, Owner, Trip};
use automerge::transaction::Transactable;
use automerge::{AutoCommit, ObjType, ReadDoc, ROOT};
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

    pub fn set_item_state(&mut self, id: Uuid, state: ItemState) -> Result<(), DocError> {
        let mut item = self
            .items()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or(DocError::NoSuchItem(id))?;
        item.state = state;
        self.add_item(&item)
    }

    pub fn merge_generated(
        &mut self,
        category: Category,
        generated: &[GeneratedItem],
    ) -> Result<MergeReport, DocError> {
        let existing: Vec<(Category, String)> = self
            .items()?
            .into_iter()
            .map(|i| (i.category, normalise(&i.name)))
            .collect();

        let mut report = MergeReport::default();

        for g in generated {
            let key = (category, normalise(&g.name));
            if existing.contains(&key) {
                report.skipped_existing += 1;
                continue;
            }
            let item = Item {
                id: Uuid::new_v4(),
                name: g.name.clone(),
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
            travellers: vec![],
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
    fn same_name_in_a_different_category_is_a_distinct_item() {
        let mut doc = TripDocument::new(&a_trip());
        doc.merge_generated(Category::Clothing, &[generated("Adapter")])
            .unwrap();
        let report = doc
            .merge_generated(Category::Electronics, &[generated("Adapter")])
            .unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(doc.items().unwrap().len(), 2);
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
}
