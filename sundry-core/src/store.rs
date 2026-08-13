use crate::document::{DocError, TripDocument};
use crate::model::TravelWindow;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(String),
    #[error("no trip with id {0}")]
    NotFound(Uuid),
    #[error("document error: {0}")]
    Document(String),
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e.to_string())
    }
}

impl From<DocError> for StoreError {
    fn from(e: DocError) -> Self {
        StoreError::Document(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TripIndexEntry {
    pub id: Uuid,
    pub title: String,
    pub year: i32,
    pub month: u32,
}

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Store { root }
    }

    fn trips_dir(&self) -> PathBuf {
        self.root.join("trips")
    }

    fn path_for(&self, id: Uuid) -> PathBuf {
        self.trips_dir().join(format!("{id}.automerge"))
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    pub fn save(&self, doc: &mut TripDocument) -> Result<(), StoreError> {
        std::fs::create_dir_all(self.trips_dir())?;
        let trip = doc.trip()?;
        std::fs::write(self.path_for(trip.id), doc.save())?;

        let mut index = self.read_index()?;
        index.retain(|e| e.id != trip.id);
        let (year, month) = match trip.window {
            TravelWindow::Month { year, month } => (year, month),
            TravelWindow::Dates { start, .. } => {
                use chrono::Datelike;
                (start.year(), start.month())
            }
        };
        index.push(TripIndexEntry {
            id: trip.id,
            title: trip.title,
            year,
            month,
        });
        self.write_index(&index)
    }

    pub fn load(&self, id: Uuid) -> Result<TripDocument, StoreError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Err(StoreError::NotFound(id));
        }
        Ok(TripDocument::load(&std::fs::read(path)?)?)
    }

    pub fn list(&self) -> Result<Vec<TripIndexEntry>, StoreError> {
        self.read_index()
    }

    pub fn delete(&self, id: Uuid) -> Result<(), StoreError> {
        let path = self.path_for(id);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let mut index = self.read_index()?;
        index.retain(|e| e.id != id);
        self.write_index(&index)
    }

    fn read_index(&self) -> Result<Vec<TripIndexEntry>, StoreError> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(path)?;
        serde_json::from_str(&raw).map_err(|e| StoreError::Io(e.to_string()))
    }

    fn write_index(&self, index: &[TripIndexEntry]) -> Result<(), StoreError> {
        std::fs::create_dir_all(&self.root)?;
        let raw = serde_json::to_string_pretty(index).map_err(|e| StoreError::Io(e.to_string()))?;
        std::fs::write(self.index_path(), raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sundry-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn a_trip(title: &str) -> Trip {
        Trip {
            id: Uuid::new_v4(),
            title: title.into(),
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
            purposes: vec![],
        }
    }

    #[test]
    fn saves_and_loads_a_trip() {
        let store = Store::new(temp_root());
        let trip = a_trip("Kyoto");
        let mut doc = TripDocument::new(&trip);
        store.save(&mut doc).unwrap();

        let loaded = store.load(trip.id).unwrap();
        assert_eq!(loaded.trip().unwrap().title, "Kyoto");
    }

    #[test]
    fn lists_saved_trips() {
        let store = Store::new(temp_root());
        let mut a = TripDocument::new(&a_trip("Kyoto"));
        let mut b = TripDocument::new(&a_trip("Reykjavik"));
        store.save(&mut a).unwrap();
        store.save(&mut b).unwrap();

        let titles: Vec<String> = store.list().unwrap().into_iter().map(|e| e.title).collect();
        assert_eq!(titles.len(), 2);
        assert!(titles.contains(&"Kyoto".to_string()));
    }

    #[test]
    fn list_is_empty_on_a_fresh_store() {
        let store = Store::new(temp_root());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn saving_twice_updates_rather_than_duplicating() {
        let store = Store::new(temp_root());
        let trip = a_trip("Kyoto");
        let mut doc = TripDocument::new(&trip);
        store.save(&mut doc).unwrap();
        doc.add_item(&Item::manual("Passport", Category::Documents))
            .unwrap();
        store.save(&mut doc).unwrap();

        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(store.load(trip.id).unwrap().items().unwrap().len(), 1);
    }

    #[test]
    fn loading_an_unknown_trip_errors() {
        let store = Store::new(temp_root());
        assert!(matches!(
            store.load(Uuid::new_v4()),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn deletes_a_trip() {
        let store = Store::new(temp_root());
        let trip = a_trip("Kyoto");
        let mut doc = TripDocument::new(&trip);
        store.save(&mut doc).unwrap();
        store.delete(trip.id).unwrap();
        assert!(store.list().unwrap().is_empty());
    }
}
