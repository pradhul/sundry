use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use sundry_core::climate::{ClimateError, ClimateNormals, ClimateProvider};
use sundry_core::document::TripDocument;
use sundry_core::facts::FactsTable;
use sundry_core::generator::Generator;
use sundry_core::llm::{LlmBackend, LlmError};
use sundry_core::model::{
    Destination, Item, ItemState, Owner, Purpose, TravelWindow, Traveller, Trip,
};
use sundry_core::store::{Store, TripIndexEntry};
use uuid::Uuid;

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SundryError {
    #[error("{message}")]
    Core { message: String },
    #[error("invalid id: {message}")]
    BadId { message: String },
}

/// A Swift callback that throws something other than `SundryError` must land
/// as an ordinary error for that one call (e.g. a single category), not abort
/// the whole generation.
impl From<uniffi::UnexpectedUniFFICallbackError> for SundryError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        SundryError::Core { message: e.reason }
    }
}

fn core_err<E: std::fmt::Display>(e: E) -> SundryError {
    SundryError::Core {
        message: e.to_string(),
    }
}

fn parse_id(raw: &str) -> Result<Uuid, SundryError> {
    Uuid::parse_str(raw).map_err(|e| SundryError::BadId {
        message: e.to_string(),
    })
}

/// Implemented in Swift over URLSession + Open-Meteo.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait ClimateTransport: Send + Sync {
    async fn normals(
        &self,
        city: String,
        country_code: String,
        month: u32,
    ) -> Option<ClimateNormalsDto>;
}

/// Implemented in Swift over FoundationModels.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait LlmTransport: Send + Sync {
    async fn generate(&self, prompt: String) -> Result<String, SundryError>;
}

#[derive(uniffi::Record, Clone, Copy)]
pub struct ClimateNormalsDto {
    pub avg_high_c: f64,
    pub avg_low_c: f64,
    pub precipitation_mm: f64,
    pub daylight_hours: f64,
}

struct ClimateAdapter(Arc<dyn ClimateTransport>);

#[async_trait::async_trait]
impl ClimateProvider for ClimateAdapter {
    async fn normals(
        &self,
        city: String,
        country_code: String,
        month: u32,
    ) -> Result<ClimateNormals, ClimateError> {
        match self.0.normals(city, country_code, month).await {
            Some(d) => Ok(ClimateNormals {
                avg_high_c: d.avg_high_c,
                avg_low_c: d.avg_low_c,
                precipitation_mm: d.precipitation_mm,
                daylight_hours: d.daylight_hours,
            }),
            None => Err(ClimateError::Unreachable),
        }
    }
}

struct LlmAdapter(Arc<dyn LlmTransport>);

#[async_trait::async_trait]
impl LlmBackend for LlmAdapter {
    async fn generate(&self, prompt: String) -> Result<String, LlmError> {
        self.0
            .generate(prompt)
            .await
            .map_err(|e| LlmError::Failed(e.to_string()))
    }
}

#[derive(uniffi::Record)]
pub struct TripSummaryDto {
    pub id: String,
    pub title: String,
    pub year: i32,
    pub month: u32,
}

impl From<TripIndexEntry> for TripSummaryDto {
    fn from(e: TripIndexEntry) -> Self {
        TripSummaryDto {
            id: e.id.to_string(),
            title: e.title,
            year: e.year,
            month: e.month,
        }
    }
}

#[derive(uniffi::Enum, Clone, Copy, PartialEq, Eq)]
pub enum ItemStateDto {
    Todo,
    Packed,
    NotNeeded,
}

impl From<ItemStateDto> for ItemState {
    fn from(s: ItemStateDto) -> Self {
        match s {
            ItemStateDto::Todo => ItemState::Todo,
            ItemStateDto::Packed => ItemState::Packed,
            ItemStateDto::NotNeeded => ItemState::NotNeeded,
        }
    }
}

impl From<ItemState> for ItemStateDto {
    fn from(s: ItemState) -> Self {
        match s {
            ItemState::Todo => ItemStateDto::Todo,
            ItemState::Packed => ItemStateDto::Packed,
            ItemState::NotNeeded => ItemStateDto::NotNeeded,
        }
    }
}

#[derive(uniffi::Record)]
pub struct ItemDto {
    pub id: String,
    pub name: String,
    pub quantity: u32,
    pub category: String,
    pub state: ItemStateDto,
    pub rationale: Option<String>,
    pub essential: bool,
    pub owner_name: Option<String>,
}

fn item_to_dto(item: Item, trip: &Trip) -> ItemDto {
    let owner_name = match item.owner {
        Owner::Shared => None,
        Owner::Traveller(id) => trip
            .travellers
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.clone()),
    };
    ItemDto {
        id: item.id.to_string(),
        name: item.name,
        quantity: item.quantity,
        category: item.category.label().to_string(),
        state: item.state.into(),
        rationale: item.rationale,
        essential: item.essential,
        owner_name,
    }
}

#[derive(uniffi::Record)]
pub struct TripFactsDto {
    pub city: String,
    pub country_code: String,
    pub country_name: Option<String>,
    pub plug_types: Vec<String>,
    pub voltage: Option<u32>,
    pub frequency: Option<u32>,
    pub currency: Option<String>,
}

#[derive(uniffi::Record)]
pub struct CategoryOutcomeDto {
    pub category: String,
    pub added: u32,
    pub error: Option<String>,
}

#[derive(uniffi::Record)]
pub struct GenerationOutcomeDto {
    pub climate_grounded: bool,
    pub per_category: Vec<CategoryOutcomeDto>,
}

#[derive(uniffi::Object)]
pub struct SundryCore {
    store: Store,
    facts: FactsTable,
    climate: Arc<dyn ClimateTransport>,
    llm: Arc<dyn LlmTransport>,
    #[allow(dead_code)]
    open: Mutex<Option<(Uuid, TripDocument)>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl SundryCore {
    #[uniffi::constructor]
    pub fn new(
        storage_root: String,
        climate: Arc<dyn ClimateTransport>,
        llm: Arc<dyn LlmTransport>,
    ) -> Arc<Self> {
        Arc::new(SundryCore {
            store: Store::new(PathBuf::from(storage_root)),
            facts: FactsTable::load(),
            climate,
            llm,
            open: Mutex::new(None),
        })
    }

    pub fn list_trips(&self) -> Result<Vec<TripSummaryDto>, SundryError> {
        Ok(self
            .store
            .list()
            .map_err(core_err)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn create_trip(
        &self,
        title: String,
        city: String,
        country_code: String,
        year: i32,
        month: u32,
        nights: u32,
        traveller_names: Vec<String>,
        purposes: Vec<String>,
    ) -> Result<String, SundryError> {
        let trip = Trip {
            id: Uuid::new_v4(),
            title,
            destinations: vec![Destination {
                country_code: country_code.to_uppercase(),
                city,
            }],
            window: TravelWindow::Month { year, month },
            nights,
            travellers: traveller_names
                .into_iter()
                .map(|name| Traveller {
                    id: Uuid::new_v4(),
                    name,
                })
                .collect(),
            purposes: purposes.iter().filter_map(|p| parse_purpose(p)).collect(),
        };
        let id = trip.id;
        let mut doc = TripDocument::new(&trip);
        self.store.save(&mut doc).map_err(core_err)?;
        Ok(id.to_string())
    }

    pub fn load_items(&self, trip_id: String) -> Result<Vec<ItemDto>, SundryError> {
        let id = parse_id(&trip_id)?;
        let doc = self.store.load(id).map_err(core_err)?;
        let trip = doc.trip().map_err(core_err)?;
        Ok(doc
            .items()
            .map_err(core_err)?
            .into_iter()
            .map(|i| item_to_dto(i, &trip))
            .collect())
    }

    pub fn load_trip_facts(&self, trip_id: String) -> Result<TripFactsDto, SundryError> {
        let id = parse_id(&trip_id)?;
        let doc = self.store.load(id).map_err(core_err)?;
        let trip = doc.trip().map_err(core_err)?;
        let dest = trip
            .destinations
            .first()
            .cloned()
            .unwrap_or(Destination {
                country_code: String::new(),
                city: String::new(),
            });
        let facts = self.facts.get(&dest.country_code);
        Ok(TripFactsDto {
            city: dest.city,
            country_code: dest.country_code,
            country_name: facts.map(|f| f.name.clone()),
            plug_types: facts.map(|f| f.plug_types.clone()).unwrap_or_default(),
            voltage: facts.map(|f| f.voltage),
            frequency: facts.map(|f| f.frequency),
            currency: facts.map(|f| f.currency.clone()),
        })
    }

    pub fn ensure_travellers(
        &self,
        trip_id: String,
        names: Vec<String>,
    ) -> Result<(), SundryError> {
        let tid = parse_id(&trip_id)?;
        let mut doc = self.store.load(tid).map_err(core_err)?;
        doc.ensure_travellers(&names).map_err(core_err)?;
        self.store.save(&mut doc).map_err(core_err)
    }

    pub fn dedupe_trip(&self, trip_id: String) -> Result<u32, SundryError> {
        let tid = parse_id(&trip_id)?;
        let mut doc = self.store.load(tid).map_err(core_err)?;
        let removed = doc.dedupe_by_normalised_name().map_err(core_err)?;
        self.store.save(&mut doc).map_err(core_err)?;
        Ok(removed as u32)
    }

    pub fn set_item_state(
        &self,
        trip_id: String,
        item_id: String,
        state: ItemStateDto,
    ) -> Result<(), SundryError> {
        let tid = parse_id(&trip_id)?;
        let iid = parse_id(&item_id)?;
        let mut doc = self.store.load(tid).map_err(core_err)?;
        doc.set_item_state(iid, state.into()).map_err(core_err)?;
        self.store.save(&mut doc).map_err(core_err)
    }

    pub fn set_item_packed_for(
        &self,
        trip_id: String,
        item_id: String,
        traveller_name: String,
    ) -> Result<(), SundryError> {
        let tid = parse_id(&trip_id)?;
        let iid = parse_id(&item_id)?;
        let mut doc = self.store.load(tid).map_err(core_err)?;
        doc.set_item_packed_for(iid, &traveller_name)
            .map_err(core_err)?;
        self.store.save(&mut doc).map_err(core_err)
    }

    pub fn delete_trip(&self, trip_id: String) -> Result<(), SundryError> {
        let tid = parse_id(&trip_id)?;
        self.store.delete(tid).map_err(core_err)
    }

    pub fn add_manual_item(
        &self,
        trip_id: String,
        name: String,
        category: String,
        quantity: u32,
        essential: bool,
    ) -> Result<String, SundryError> {
        let tid = parse_id(&trip_id)?;
        let cat = sundry_core::model::Category::from_label(&category).ok_or_else(|| {
            SundryError::Core {
                message: format!("unknown category: {category}"),
            }
        })?;
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err(SundryError::Core {
                message: "item name is empty".into(),
            });
        }
        let quantity = quantity.clamp(1, 20);
        let mut doc = self.store.load(tid).map_err(core_err)?;
        let key = sundry_core::llm::normalise(&trimmed);
        if doc
            .items()
            .map_err(core_err)?
            .iter()
            .any(|i| sundry_core::llm::normalise(&i.name) == key)
        {
            return Err(SundryError::Core {
                message: format!("“{trimmed}” is already on the list"),
            });
        }
        let mut item = Item::manual(&trimmed, cat);
        item.quantity = quantity;
        item.essential = essential;
        let id = item.id;
        doc.add_item(&item).map_err(core_err)?;
        self.store.save(&mut doc).map_err(core_err)?;
        Ok(id.to_string())
    }

    pub fn export_trip(&self, trip_id: String) -> Result<Vec<u8>, SundryError> {
        let tid = parse_id(&trip_id)?;
        let mut doc = self.store.load(tid).map_err(core_err)?;
        Ok(doc.save())
    }

    /// Import a shared `.automerge` file. Merges into an existing trip with the same id,
    /// otherwise saves it as a new trip on this Mac.
    pub fn import_trip(&self, bytes: Vec<u8>) -> Result<String, SundryError> {
        let mut incoming = TripDocument::load(&bytes).map_err(core_err)?;
        let trip = incoming.trip().map_err(core_err)?;
        let id = trip.id;
        match self.store.load(id) {
            Ok(mut local) => {
                local.merge(&mut incoming).map_err(core_err)?;
                local.dedupe_by_normalised_name().map_err(core_err)?;
                self.store.save(&mut local).map_err(core_err)?;
            }
            Err(_) => {
                self.store.save(&mut incoming).map_err(core_err)?;
            }
        }
        Ok(id.to_string())
    }

    pub async fn generate(&self, trip_id: String) -> Result<GenerationOutcomeDto, SundryError> {
        let id = parse_id(&trip_id)?;
        let mut doc = self.store.load(id).map_err(core_err)?;

        let climate = ClimateAdapter(self.climate.clone());
        let llm = LlmAdapter(self.llm.clone());
        let generator = Generator::new(&self.facts, &climate, &llm);

        let outcome = generator.generate_all(&mut doc).await.map_err(core_err)?;
        self.store.save(&mut doc).map_err(core_err)?;

        Ok(GenerationOutcomeDto {
            climate_grounded: outcome.climate_grounded,
            per_category: outcome
                .per_category
                .into_iter()
                .map(|c| CategoryOutcomeDto {
                    category: c.category.label().to_string(),
                    added: c.added as u32,
                    error: c.error,
                })
                .collect(),
        })
    }
}

fn parse_purpose(raw: &str) -> Option<Purpose> {
    match raw {
        "business" => Some(Purpose::Business),
        "leisure" => Some(Purpose::Leisure),
        "hiking" => Some(Purpose::Hiking),
        "beach" => Some(Purpose::Beach),
        "city" => Some(Purpose::City),
        _ => None,
    }
}

/// Referenced so `Category` stays linked; also used by the app to render section order.
#[uniffi::export]
pub fn category_order() -> Vec<String> {
    sundry_core::model::Category::ALL
        .iter()
        .map(|c| c.label().to_string())
        .collect()
}
