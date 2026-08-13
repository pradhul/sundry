use crate::climate::ClimateProvider;
use crate::document::{DocError, TripDocument};
use crate::facts::FactsTable;
use crate::llm::{parse_and_validate, LlmBackend};
use crate::model::Category;
use crate::prompt::{build_prompt, GroundedContext};

pub struct CategoryOutcome {
    pub category: Category,
    pub added: usize,
    pub error: Option<String>,
}

pub struct GenerationOutcome {
    pub climate_grounded: bool,
    pub per_category: Vec<CategoryOutcome>,
}

pub struct Generator<'a> {
    facts: &'a FactsTable,
    climate: &'a dyn ClimateProvider,
    llm: &'a dyn LlmBackend,
}

impl<'a> Generator<'a> {
    pub fn new(
        facts: &'a FactsTable,
        climate: &'a dyn ClimateProvider,
        llm: &'a dyn LlmBackend,
    ) -> Self {
        Generator {
            facts,
            climate,
            llm,
        }
    }

    pub async fn generate_all(
        &self,
        doc: &mut TripDocument,
    ) -> Result<GenerationOutcome, DocError> {
        let trip = doc.trip()?;

        let country_facts = trip
            .destinations
            .iter()
            .filter_map(|d| self.facts.get(&d.country_code).cloned())
            .collect();

        let primary = trip.destinations.first();
        let normals = match primary {
            Some(dest) => self
                .climate
                .normals(
                    dest.city.clone(),
                    dest.country_code.clone(),
                    trip.window.month(),
                )
                .await
                .ok(),
            None => None,
        };

        let ctx = GroundedContext {
            trip,
            facts: country_facts,
            climate: normals,
        };

        let mut per_category = Vec::with_capacity(Category::ALL.len());

        for category in Category::ALL {
            let prompt = build_prompt(&ctx, category);
            let outcome = match self.llm.generate(prompt).await {
                Err(e) => CategoryOutcome {
                    category,
                    added: 0,
                    error: Some(e.to_string()),
                },
                Ok(raw) => match parse_and_validate(&raw) {
                    Err(e) => CategoryOutcome {
                        category,
                        added: 0,
                        error: Some(e.to_string()),
                    },
                    Ok(items) => {
                        let report = doc.merge_generated(category, &items)?;
                        CategoryOutcome {
                            category,
                            added: report.added,
                            error: None,
                        }
                    }
                },
            };
            per_category.push(outcome);
        }

        Ok(GenerationOutcome {
            climate_grounded: ctx.climate.is_some(),
            per_category,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::climate::{ClimateNormals, FakeClimateProvider};
    use crate::document::TripDocument;
    use crate::llm::{FakeLlmBackend, LlmError};
    use crate::model::*;
    use futures::executor::block_on;
    use uuid::Uuid;

    const ONE_ITEM: &str =
        r#"{"items":[{"name":"Charger","quantity":1,"rationale":"r","essential":false}]}"#;

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

    fn normals() -> ClimateNormals {
        ClimateNormals {
            avg_high_c: 17.0,
            avg_low_c: 7.0,
            precipitation_mm: 60.0,
            daylight_hours: 10.0,
        }
    }

    #[test]
    fn generates_every_category() {
        let facts = crate::facts::FactsTable::load();
        let climate = FakeClimateProvider::returning(normals());
        let llm = FakeLlmBackend::replaying(vec![ONE_ITEM.to_string()]);
        let generator = Generator::new(&facts, &climate, &llm);
        let mut doc = TripDocument::new(&a_trip());

        let outcome = block_on(generator.generate_all(&mut doc)).unwrap();

        assert_eq!(outcome.per_category.len(), 11);
        assert!(outcome.climate_grounded);
        assert!(outcome.per_category.iter().all(|c| c.error.is_none()));
    }

    #[test]
    fn reports_when_climate_grounding_is_missing() {
        let facts = crate::facts::FactsTable::load();
        let climate = FakeClimateProvider::failing();
        let llm = FakeLlmBackend::replaying(vec![ONE_ITEM.to_string()]);
        let generator = Generator::new(&facts, &climate, &llm);
        let mut doc = TripDocument::new(&a_trip());

        let outcome = block_on(generator.generate_all(&mut doc)).unwrap();

        assert!(!outcome.climate_grounded, "must report degraded grounding");
        assert!(
            outcome.per_category.iter().any(|c| c.added > 0),
            "must still generate a list without climate"
        );
    }

    #[test]
    fn a_failing_category_does_not_abort_the_others() {
        let facts = crate::facts::FactsTable::load();
        let climate = FakeClimateProvider::returning(normals());
        let llm = FakeLlmBackend::replaying(vec!["not json".to_string(), ONE_ITEM.to_string()]);
        let generator = Generator::new(&facts, &climate, &llm);
        let mut doc = TripDocument::new(&a_trip());

        let outcome = block_on(generator.generate_all(&mut doc)).unwrap();

        assert_eq!(
            outcome
                .per_category
                .iter()
                .filter(|c| c.error.is_some())
                .count(),
            1
        );
        assert_eq!(
            outcome
                .per_category
                .iter()
                .filter(|c| c.error.is_none())
                .count(),
            10
        );
    }

    #[test]
    fn unavailable_model_fails_every_category_without_panicking() {
        let facts = crate::facts::FactsTable::load();
        let climate = FakeClimateProvider::returning(normals());
        let llm = FakeLlmBackend::failing(LlmError::Unavailable);
        let generator = Generator::new(&facts, &climate, &llm);
        let mut doc = TripDocument::new(&a_trip());

        let outcome = block_on(generator.generate_all(&mut doc)).unwrap();

        assert!(outcome.per_category.iter().all(|c| c.error.is_some()));
        assert!(doc.items().unwrap().is_empty());
    }

    #[test]
    fn unknown_country_code_still_generates_using_remaining_facts() {
        let facts = crate::facts::FactsTable::load();
        let climate = FakeClimateProvider::returning(normals());
        let llm = FakeLlmBackend::replaying(vec![ONE_ITEM.to_string()]);
        let generator = Generator::new(&facts, &climate, &llm);
        let mut trip = a_trip();
        trip.destinations = vec![Destination {
            country_code: "ZZ".into(),
            city: "Nowhere".into(),
        }];
        let mut doc = TripDocument::new(&trip);

        let outcome = block_on(generator.generate_all(&mut doc)).unwrap();
        assert!(outcome.per_category.iter().any(|c| c.added > 0));
    }
}
