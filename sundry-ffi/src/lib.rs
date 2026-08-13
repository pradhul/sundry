use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use sundry_core::climate::{ClimateError, ClimateNormals, ClimateProvider};
use sundry_core::document::TripDocument;
use sundry_core::facts::FactsTable;
use sundry_core::generator::Generator;
use sundry_core::llm::{LlmBackend, LlmError};
use sundry_core::model::{
    Destination, Item, ItemState, Purpose, TravelWindow, Traveller, Trip,
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
}

impl From<Item> for ItemDto {
    fn from(i: Item) -> Self {
        ItemDto {
            id: i.id.to_string(),
            name: i.name,
            quantity: i.quantity,
            category: i.category.label().to_string(),
            state: i.state.into(),
            rationale: i.rationale,
            essential: i.essential,
        }
    }
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
        Ok(doc
            .items()
            .map_err(core_err)?
            .into_iter()
            .map(Into::into)
            .collect())
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
