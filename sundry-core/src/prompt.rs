use crate::climate::ClimateNormals;
use crate::facts::{CountryFacts, DrivingSide, TapWater};
use crate::model::{Category, Purpose, Trip};

pub struct GroundedContext {
    pub trip: Trip,
    pub facts: Vec<CountryFacts>,
    pub climate: Option<ClimateNormals>,
}

fn month_name(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    NAMES[(month.clamp(1, 12) - 1) as usize]
}

fn purpose_label(p: &Purpose) -> &'static str {
    match p {
        Purpose::Business => "business",
        Purpose::Leisure => "leisure",
        Purpose::Hiking => "hiking and outdoors",
        Purpose::Beach => "beach",
        Purpose::City => "city sightseeing",
    }
}

fn category_scope(category: Category) -> &'static str {
    match category {
        Category::Documents => {
            "This category means travel papers only: passport, tickets, insurance, copies of IDs. Not clothing, shoes, cameras, or cash."
        }
        Category::Money => {
            "This category means cash, cards, and a money belt. Not shoes, cameras, or clothing unless it is a wallet."
        }
        Category::Electronics => {
            "This category means phones, chargers, adapters, cameras, and cables. Not clothing or shoes."
        }
        Category::Clothing => {
            "This category means garments: layers, rainwear, sleepwear. Not shoes, gadgets, or papers."
        }
        Category::Footwear => {
            "This category means shoes, sandals, boots, and socks. Not garments, gadgets, or papers."
        }
        Category::Toiletries => {
            "This category means wash kit: toothbrush, sunscreen, deodorant. Not clothing or gadgets."
        }
        Category::Health => {
            "This category means medication, first aid, and prescriptions. Not everyday wash kit or clothing."
        }
        Category::Bags => {
            "This category means luggage, packing cubes, and organisers. Not the items that go inside them."
        }
        Category::Comfort => {
            "This category means pillows, books, snacks, and sleep aids. Not clothing or gadgets already listed elsewhere."
        }
        Category::Activity => {
            "This category means gear for the trip's purposes (hike, beach, business). Not everyday clothing or shoes."
        }
        Category::Misc => {
            "This category means leftovers that fit nowhere else. Do not repeat anything already listed."
        }
    }
}

struct RelevantFacts {
    electricity: bool,
    currency: bool,
    driving: bool,
    tap_water: bool,
    climate: bool,
}

/// Each category only sees the grounding facts that can change what goes in it;
/// irrelevant facts otherwise leak into item names.
fn relevant_facts(category: Category) -> RelevantFacts {
    let none = RelevantFacts {
        electricity: false,
        currency: false,
        driving: false,
        tap_water: false,
        climate: false,
    };
    match category {
        Category::Electronics => RelevantFacts {
            electricity: true,
            ..none
        },
        Category::Money => RelevantFacts {
            currency: true,
            ..none
        },
        Category::Documents => RelevantFacts {
            currency: true,
            driving: true,
            ..none
        },
        Category::Clothing | Category::Footwear | Category::Activity | Category::Bags => {
            RelevantFacts {
                climate: true,
                ..none
            }
        }
        Category::Toiletries | Category::Comfort | Category::Misc => RelevantFacts {
            climate: true,
            ..none
        },
        Category::Health => RelevantFacts {
            tap_water: true,
            climate: true,
            ..none
        },
    }
}

pub fn build_prompt(
    ctx: &GroundedContext,
    category: Category,
    already_listed: &[String],
) -> String {
    let mut out = String::new();

    out.push_str("Trip facts (authoritative — do not contradict these):\n");

    for dest in &ctx.trip.destinations {
        out.push_str(&format!(
            "- Destination: {}, {}\n",
            dest.city, dest.country_code
        ));
    }
    out.push_str(&format!(
        "- Travel month: {}\n",
        month_name(ctx.trip.window.month())
    ));
    out.push_str(&format!("- Nights: {}\n", ctx.trip.nights));
    out.push_str(&format!("- Travellers: {}\n", ctx.trip.travellers.len()));

    if !ctx.trip.purposes.is_empty() {
        let purposes: Vec<&str> = ctx.trip.purposes.iter().map(purpose_label).collect();
        out.push_str(&format!("- Purpose: {}\n", purposes.join(", ")));
    }

    let rel = relevant_facts(category);
    for facts in &ctx.facts {
        if rel.electricity {
            out.push_str(&format!(
                "- {} mains electricity: {}V, {}Hz, socket types {}\n",
                facts.name,
                facts.voltage,
                facts.frequency,
                facts.plug_types.join(", ")
            ));
        }
        if rel.currency {
            out.push_str(&format!("- {} currency: {}\n", facts.name, facts.currency));
        }
        if rel.driving {
            out.push_str(&format!(
                "- {} drives on the {}\n",
                facts.name,
                match facts.drives_on {
                    DrivingSide::Left => "left",
                    DrivingSide::Right => "right",
                }
            ));
        }
        if rel.tap_water {
            out.push_str(&format!(
                "- {} tap water: {}\n",
                facts.name,
                match facts.tap_water {
                    TapWater::Potable => "safe to drink",
                    TapWater::NotPotable => "not safe to drink",
                    TapWater::Caution => "varies by region, treat with caution",
                }
            ));
        }
    }

    if rel.climate {
        match &ctx.climate {
            Some(c) => out.push_str(&format!(
                "- Climate normals for {}: average high {:.0}C, average low {:.0}C, \
             {:.0}mm precipitation over the month, about {:.0} hours of daylight\n",
                month_name(ctx.trip.window.month()),
                c.avg_high_c,
                c.avg_low_c,
                c.precipitation_mm,
                c.daylight_hours
            )),
            None => out.push_str(
                "- Climate data is unavailable for this destination. Do not guess the \
             weather; suggest only items that hold regardless of conditions.\n",
            ),
        }
    }

    out.push_str(&format!("\n{}\n", category_scope(category)));

    if !already_listed.is_empty() {
        out.push_str("\nAlready on the packing list (do not repeat these names in any form):\n");
        for name in already_listed {
            out.push_str(&format!("- {name}\n"));
        }
    }

    out.push_str(&format!(
        "\nTask: list at most 5 items in the category \"{}\" that these travellers should pack \
         for this trip. Only this category — refuse anything that belongs elsewhere. Use the \
         facts above rather than your own knowledge. Each name must be a plain, real product \
         name a shop would use (for example \"Rain jacket\", \"Thermal base layer\", \
         \"Toothbrush\"). Never put trip facts such as voltage, plug or socket types, currency \
         codes, driving side, tap water, or climate numbers into a name unless the fact is the \
         product itself (a plug adapter, local cash, sunscreen SPF). Put the trip-specific \
         justification in the one-sentence reason instead. List each kind of item once; do not \
         list variations of the same item. Mark an item essential only if the trip fails \
         without it.\n",
        category.label()
    ));

    out.push_str(
        "Only use a supplied fact when it genuinely changes what to pack in this category; \
         ignore it otherwise. Name the destination country correctly when citing a fact.\n",
    );

    if category == Category::Documents {
        out.push_str(
            "Do not state visa or entry requirements, and do not say whether a visa is \
             needed. List document types to carry only.\n",
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::climate::ClimateNormals;
    use crate::facts::{CountryFacts, DrivingSide, TapWater};
    use crate::model::*;
    use uuid::Uuid;

    fn prompt(ctx: &GroundedContext, category: Category) -> String {
        build_prompt(ctx, category, &[])
    }

    fn kyoto_context(climate: Option<ClimateNormals>) -> GroundedContext {
        GroundedContext {
            trip: Trip {
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
                        name: "A".into(),
                    },
                    Traveller {
                        id: Uuid::new_v4(),
                        name: "B".into(),
                    },
                ],
                purposes: vec![Purpose::City],
            },
            facts: vec![CountryFacts {
                code: "JP".into(),
                name: "Japan".into(),
                plug_types: vec!["A".into(), "B".into()],
                voltage: 100,
                frequency: 50,
                currency: "JPY".into(),
                drives_on: DrivingSide::Left,
                tap_water: TapWater::Potable,
            }],
            climate,
        }
    }

    #[test]
    fn prompt_contains_the_grounded_socket_facts() {
        let p = prompt(&kyoto_context(None), Category::Electronics);
        assert!(p.contains("100V"), "voltage missing from prompt");
        assert!(p.contains("50Hz"), "frequency missing from prompt");
        assert!(p.contains("A, B"), "plug types missing from prompt");
    }

    #[test]
    fn prompt_contains_climate_when_available() {
        let p = prompt(
            &kyoto_context(Some(ClimateNormals {
                avg_high_c: 17.0,
                avg_low_c: 7.0,
                precipitation_mm: 60.0,
                daylight_hours: 10.0,
            })),
            Category::Clothing,
        );
        assert!(p.contains("17"), "average high missing");
        assert!(p.contains("7"), "average low missing");
        assert!(p.contains("60"), "precipitation missing");
    }

    #[test]
    fn prompt_says_climate_is_unknown_when_absent() {
        let p = prompt(&kyoto_context(None), Category::Clothing);
        assert!(
            p.to_lowercase().contains("climate data is unavailable"),
            "must state climate is unknown rather than omitting it silently"
        );
    }

    #[test]
    fn prompt_names_only_the_requested_category() {
        let p = prompt(&kyoto_context(None), Category::Footwear);
        assert!(p.contains("Footwear"));
        assert!(
            !p.contains("Toiletries"),
            "must not mention other categories"
        );
    }

    #[test]
    fn prompt_instructs_the_model_not_to_use_its_own_knowledge() {
        let p = prompt(&kyoto_context(None), Category::Electronics);
        assert!(p.contains("do not contradict"));
    }

    #[test]
    fn documents_prompt_forbids_asserting_entry_rules() {
        let p = prompt(&kyoto_context(None), Category::Documents);
        assert!(
            p.contains("Do not state visa or entry requirements"),
            "documents prompt must forbid asserting entry rules"
        );
    }

    #[test]
    fn prompt_scopes_what_the_category_means() {
        let p = prompt(&kyoto_context(None), Category::Documents);
        assert!(
            p.contains("This category means travel papers"),
            "must tell the model what belongs in this category"
        );
    }

    #[test]
    fn prompt_asks_for_concrete_attributes() {
        let p = prompt(&kyoto_context(None), Category::Toiletries);
        assert!(p.contains("SPF"));
        assert!(p.contains("at most 5"));
        assert!(p.contains("plain, real product name"));
    }

    #[test]
    fn prompt_lists_already_packed_names() {
        let p = build_prompt(
            &kyoto_context(None),
            Category::Footwear,
            &["Passport".into(), "Comfortable walking shoes".into()],
        );
        assert!(p.contains("Already on the packing list"));
        assert!(p.contains("Passport"));
        assert!(p.contains("Comfortable walking shoes"));
        assert!(p.contains("do not repeat"));
    }

    #[test]
    fn only_relevant_facts_reach_each_category() {
        let ctx = kyoto_context(None);
        let clothing = prompt(&ctx, Category::Clothing);
        assert!(!clothing.contains("100V"));
        assert!(!clothing.contains("JPY"));
        assert!(!clothing.contains("drives on"));
        assert!(!clothing.contains("tap water:"));
        let money = prompt(&ctx, Category::Money);
        assert!(money.contains("JPY"));
        assert!(!money.contains("100V"));
        let toiletries = prompt(&ctx, Category::Toiletries);
        assert!(!toiletries.contains("mains electricity"));
    }
}
