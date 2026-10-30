use crate::climate::ClimateProvider;
use crate::document::{DocError, TripDocument};
use crate::facts::FactsTable;
use crate::llm::{parse_and_validate, GeneratedItem, LlmBackend, LlmError};
use crate::model::Category;
use crate::prompt::{build_prompt, is_cold, GroundedContext};

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
            let (mut added, mut error) = match self.attempt(doc, &ctx, category).await? {
                Ok(n) => (n, None),
                Err(e) => (0, Some(e)),
            };
            let unavailable = error.as_deref() == Some(&LlmError::Unavailable.to_string());
            if added < MIN_PER_CATEGORY && !unavailable {
                if let Ok(n) = self.attempt(doc, &ctx, category).await? {
                    added += n;
                    error = None;
                }
            }
            if error.is_none() {
                let existing: Vec<String> = doc.items()?.into_iter().map(|i| i.name).collect();
                let staples = staples(&ctx, category, &existing);
                if !staples.is_empty() {
                    added += place_and_merge(doc, &ctx, category, staples)?;
                }
            }
            let outcome = CategoryOutcome {
                category,
                added,
                error,
            };
            per_category.push(outcome);
        }

        Ok(GenerationOutcome {
            climate_grounded: ctx.climate.is_some(),
            per_category,
        })
    }
}

impl Generator<'_> {
    /// One generation attempt for a category. The inner error is a model or
    /// validation failure; the outer one is a document failure.
    async fn attempt(
        &self,
        doc: &mut TripDocument,
        ctx: &GroundedContext,
        category: Category,
    ) -> Result<Result<usize, String>, DocError> {
        let already: Vec<String> = doc.items()?.into_iter().map(|i| i.name).collect();
        let prompt = build_prompt(ctx, category, &already);
        let raw = match self.llm.generate(prompt).await {
            Ok(raw) => raw,
            Err(e) => return Ok(Err(e.to_string())),
        };
        match parse_and_validate(&raw) {
            Ok(items) => Ok(Ok(place_and_merge(doc, ctx, category, items)?)),
            Err(e) => Ok(Err(e.to_string())),
        }
    }
}

/// Items that should come in one per day (plus a spare).
fn daily_quantity(ctx: &GroundedContext) -> u32 {
    (ctx.trip.nights + 1).clamp(1, crate::llm::MAX_QUANTITY)
}

fn is_daily_item(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("underwear") || n.contains("sock")
}

fn plug_adapter_name(ctx: &GroundedContext) -> Option<String> {
    ctx.facts
        .first()
        .map(|f| format!("Type {} plug adapter", f.plug_types.join("/")))
}

/// Deterministic fix-ups of model output: a generic plug adapter becomes the
/// destination-specific one, and per-day items scale with trip length.
pub fn specialise(
    ctx: &GroundedContext,
    category: Category,
    items: Vec<GeneratedItem>,
) -> Vec<GeneratedItem> {
    items
        .into_iter()
        .map(|mut item| {
            let lower = item.name.to_lowercase();
            if category == Category::Electronics
                && lower.contains("adapter")
                && !lower.contains("type ")
                && !lower.contains("usb")
            {
                if let Some(name) = plug_adapter_name(ctx) {
                    item.name = name;
                }
            }
            if is_daily_item(&item.name) {
                item.quantity = item.quantity.max(daily_quantity(ctx));
            }
            item
        })
        .collect()
}

fn traveller_count(ctx: &GroundedContext) -> u32 {
    (ctx.trip.travellers.len() as u32).max(1)
}

/// Purpose fit, home category, destination-specific names, and per-traveller
/// quantities, then merge each item into the category it belongs in.
/// Quantities from the model and staples are per person.
pub fn prepare(
    ctx: &GroundedContext,
    category: Category,
    items: Vec<GeneratedItem>,
) -> Vec<(Category, GeneratedItem)> {
    let travellers = traveller_count(ctx);
    items
        .into_iter()
        .filter_map(|mut item| {
            item.name = crate::placement::fit_purpose(&item.name, &ctx.trip.purposes)?;
            let target = crate::placement::home_category(&item.name).unwrap_or(category);
            let mut item = specialise(ctx, target, vec![item]).pop()?;
            if crate::placement::is_one_per_person(&item.name) {
                item.quantity = 1;
            }
            if crate::placement::is_personal(&item.name) {
                item.quantity = (item.quantity * travellers).min(crate::llm::MAX_QUANTITY);
            } else {
                item.quantity = crate::placement::shared_quantity(&item.name, travellers);
            }
            Some((target, item))
        })
        .collect()
}

fn place_and_merge(
    doc: &mut TripDocument,
    ctx: &GroundedContext,
    category: Category,
    items: Vec<GeneratedItem>,
) -> Result<usize, DocError> {
    let mut added = 0;
    for (target, item) in prepare(ctx, category, items) {
        added += doc.merge_generated(target, &[item])?.added;
    }
    Ok(added)
}

/// Retry a category once if fewer than this many items survived validation.
pub const MIN_PER_CATEGORY: usize = 3;

fn staple(name: &str, rationale: String, essential: bool) -> GeneratedItem {
    GeneratedItem {
        name: name.to_string(),
        quantity: 1,
        rationale,
        essential,
    }
}

/// Deterministic basics the on-device model often forgets. Each is only added
/// if nothing in the list already covers it (checked by keyword).
pub fn staples(
    ctx: &GroundedContext,
    category: Category,
    existing: &[String],
) -> Vec<GeneratedItem> {
    let cold = is_cold(ctx.climate.as_ref());
    let mut out = Vec::new();
    match category {
        Category::Electronics => {
            for f in &ctx.facts {
                out.push((
                    vec!["adapter", "converter"],
                    staple(
                        &format!("Type {} plug adapter", f.plug_types.join("/")),
                        format!(
                            "{} uses type {} sockets at {}V.",
                            f.name,
                            f.plug_types.join("/"),
                            f.voltage
                        ),
                        true,
                    ),
                ));
            }
            out.push((
                vec!["charger"],
                staple(
                    "Phone charger",
                    "Keeps phones topped up for maps and tickets.".into(),
                    true,
                ),
            ));
        }
        Category::Clothing => {
            out.push((
                vec!["underwear"],
                staple(
                    "Underwear",
                    format!("One per day for {} nights.", ctx.trip.nights),
                    true,
                ),
            ));
            out.push((
                vec!["sleepwear", "pajama", "pyjama", "nightwear"],
                staple(
                    "Sleepwear",
                    "Something warm to sleep in each night.".into(),
                    false,
                ),
            ));
            out.push((
                vec!["trouser", "pants", "jeans", "chino", "slacks"],
                staple(
                    "Trousers",
                    "Everyday bottoms for city walking.".into(),
                    false,
                ),
            ));
            out.push((
                vec!["shirt", "top", "tee", "blouse"],
                staple(
                    "Long-sleeve tops",
                    "Everyday layers to wear under a jumper or coat.".into(),
                    false,
                ),
            ));
            if cold {
                let why = |w: &str| {
                    format!(
                        "Average lows around {:.0}C call for {w}.",
                        ctx.climate.map(|c| c.avg_low_c).unwrap_or(0.0)
                    )
                };
                out.push((
                    vec!["coat", "parka", "down jacket", "puffer"],
                    staple("Warm coat", why("a warm coat"), true),
                ));
                out.push((
                    vec!["sweater", "jumper", "fleece", "cardigan"],
                    staple("Sweater", why("an insulating mid layer"), false),
                ));
                out.push((
                    vec!["glove", "mitten"],
                    staple("Gloves", why("gloves"), false),
                ));
                out.push((vec!["scarf"], staple("Scarf", why("a scarf"), false)));
                out.push((
                    vec!["hat", "beanie"],
                    staple("Warm hat", why("a warm hat"), false),
                ));
            }
        }
        Category::Footwear => {
            out.push((
                vec!["sock"],
                staple(
                    if cold { "Warm socks" } else { "Socks" },
                    format!("A fresh pair for each of {} days.", ctx.trip.nights),
                    true,
                ),
            ));
        }
        Category::Toiletries => {
            out.push((
                vec!["toothpaste"],
                staple("Toothpaste", "Pairs with your toothbrush.".into(), false),
            ));
        }
        _ => {}
    }
    let daily = daily_quantity(ctx);
    for (_, item) in out.iter_mut() {
        if is_daily_item(&item.name) {
            item.quantity = daily;
        }
    }
    let existing: Vec<String> = existing.iter().map(|n| n.to_lowercase()).collect();
    out.into_iter()
        .filter(|(keys, _)| !existing.iter().any(|e| keys.iter().any(|k| e.contains(k))))
        .map(|(_, item)| item)
        .collect()
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
        let llm = FakeLlmBackend::replaying(vec![
            "not json".to_string(),
            "not json".to_string(),
            ONE_ITEM.to_string(),
        ]);
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

    fn kyoto_ctx(cold: bool) -> GroundedContext {
        let facts = crate::facts::FactsTable::load();
        GroundedContext {
            trip: a_trip(),
            facts: vec![facts.get("JP").unwrap().clone()],
            climate: Some(ClimateNormals {
                avg_high_c: if cold { 10.0 } else { 30.0 },
                avg_low_c: if cold { 1.0 } else { 22.0 },
                precipitation_mm: 70.0,
                daylight_hours: 10.0,
            }),
        }
    }

    fn names(items: Vec<GeneratedItem>) -> Vec<String> {
        items.into_iter().map(|i| i.name).collect()
    }

    #[test]
    fn electronics_staples_use_the_destination_plug_types() {
        let n = names(staples(&kyoto_ctx(true), Category::Electronics, &[]));
        assert!(n.contains(&"Type A/B plug adapter".to_string()), "{n:?}");
        assert!(n.contains(&"Phone charger".to_string()));
    }

    #[test]
    fn cold_trip_gets_winter_clothing_staples() {
        let n = names(staples(&kyoto_ctx(true), Category::Clothing, &[]));
        for want in [
            "Underwear",
            "Trousers",
            "Long-sleeve tops",
            "Warm coat",
            "Gloves",
            "Scarf",
            "Warm hat",
        ] {
            assert!(n.contains(&want.to_string()), "missing {want}: {n:?}");
        }
        let hot = names(staples(&kyoto_ctx(false), Category::Clothing, &[]));
        assert!(!hot.contains(&"Warm coat".to_string()));
    }

    #[test]
    fn staples_skip_what_the_model_already_listed() {
        let n = names(staples(
            &kyoto_ctx(true),
            Category::Clothing,
            &["Winter coat".into(), "Wool beanie".into()],
        ));
        assert!(!n.contains(&"Warm coat".to_string()));
        assert!(!n.contains(&"Warm hat".to_string()));
    }

    #[test]
    fn thin_category_gets_one_retry_and_staples() {
        let facts = crate::facts::FactsTable::load();
        let climate = FakeClimateProvider::returning(ClimateNormals {
            avg_high_c: 10.0,
            avg_low_c: 1.0,
            precipitation_mm: 70.0,
            daylight_hours: 10.0,
        });
        let first =
            r#"{"items":[{"name":"Rain jacket","quantity":1,"rationale":"r","essential":false}]}"#;
        let second = r#"{"items":[{"name":"Trousers","quantity":2,"rationale":"r","essential":false},{"name":"Sweater","quantity":2,"rationale":"r","essential":false}]}"#;
        let llm = FakeLlmBackend::replaying(vec![first.into(), second.into()]);
        let generator = Generator::new(&facts, &climate, &llm);
        let mut doc = TripDocument::new(&a_trip());
        block_on(generator.generate_all(&mut doc)).unwrap();
        let n: Vec<String> = doc.items().unwrap().into_iter().map(|i| i.name).collect();
        for want in [
            "Trousers",
            "Warm coat",
            "Underwear",
            "Type A/B plug adapter",
            "Warm socks",
            "Toothpaste",
        ] {
            assert!(n.contains(&want.to_string()), "missing {want}: {n:?}");
        }
    }

    fn gen(name: &str, quantity: u32) -> GeneratedItem {
        GeneratedItem {
            name: name.into(),
            quantity,
            rationale: "r".into(),
            essential: false,
        }
    }

    #[test]
    fn generic_plug_adapter_becomes_country_specific() {
        let ctx = kyoto_ctx(true);
        let out = specialise(
            &ctx,
            Category::Electronics,
            vec![
                gen("Plug adapter", 1),
                gen("Travel adapter", 1),
                gen("USB-C adapter", 1),
            ],
        );
        assert_eq!(out[0].name, "Type A/B plug adapter");
        assert_eq!(out[1].name, "Type A/B plug adapter");
        assert_eq!(out[2].name, "USB-C adapter");
    }

    #[test]
    fn daily_items_scale_with_trip_length() {
        let ctx = kyoto_ctx(true); // 7 nights
        let out = specialise(
            &ctx,
            Category::Clothing,
            vec![gen("Underwear", 1), gen("Sweater", 1)],
        );
        assert_eq!(out[0].quantity, 8);
        assert_eq!(out[1].quantity, 1);
        let socks: Vec<GeneratedItem> = staples(&ctx, Category::Footwear, &[]);
        assert_eq!(socks[0].quantity, 8);
        let under = staples(&ctx, Category::Clothing, &[]);
        assert_eq!(
            under
                .iter()
                .find(|i| i.name == "Underwear")
                .unwrap()
                .quantity,
            8
        );
    }

    #[test]
    fn prepare_moves_scales_and_filters() {
        let mut ctx = kyoto_ctx(true);
        ctx.trip.purposes = vec![Purpose::Leisure, Purpose::City];
        ctx.trip.travellers = vec![
            Traveller {
                id: Uuid::new_v4(),
                name: "A".into(),
            },
            Traveller {
                id: Uuid::new_v4(),
                name: "B".into(),
            },
            Traveller {
                id: Uuid::new_v4(),
                name: "C".into(),
            },
        ];
        let out = prepare(
            &ctx,
            Category::Misc,
            vec![
                gen("Sunscreen SPF 30", 1),
                gen("Headlamp", 1),
                gen("Compact umbrella", 1),
                gen("Travel adapter", 1),
                gen("Reusable water bottle", 2),
                gen("Toothbrush", 2),
                gen("Underwear", 1),
            ],
        );
        let find = |n: &str| out.iter().find(|(_, i)| i.name == n).cloned();
        assert_eq!(find("Sunscreen SPF 30").unwrap().0, Category::Toiletries);
        assert!(find("Headlamp").is_none());
        assert_eq!(find("Compact umbrella").unwrap().1.quantity, 1);
        let adapter = find("Type A/B plug adapter").unwrap();
        assert_eq!((adapter.0, adapter.1.quantity), (Category::Electronics, 1));
        assert_eq!(find("Reusable water bottle").unwrap().1.quantity, 3);
        assert_eq!(find("Toothbrush").unwrap().1.quantity, 3);
        // 7 nights + 1 spare, times 3 travellers, capped at MAX_QUANTITY.
        assert_eq!(find("Underwear").unwrap().1.quantity, 20);
    }

    #[test]
    fn traveller_count_drives_quantity_not_a_constant() {
        let mut ctx = kyoto_ctx(true);
        ctx.trip.travellers = vec![Traveller {
            id: Uuid::new_v4(),
            name: "Solo".into(),
        }];
        let out = prepare(&ctx, Category::Clothing, vec![gen("Warm coat", 1)]);
        assert_eq!(out[0].1.quantity, 1);
        ctx.trip.travellers.push(Traveller {
            id: Uuid::new_v4(),
            name: "Two".into(),
        });
        let out = prepare(&ctx, Category::Clothing, vec![gen("Warm coat", 1)]);
        assert_eq!(out[0].1.quantity, 2);
    }
}
