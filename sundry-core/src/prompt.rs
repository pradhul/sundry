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

pub fn build_prompt(ctx: &GroundedContext, category: Category, already_listed: &[String]) -> String {
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

    for facts in &ctx.facts {
        out.push_str(&format!(
            "- {} mains electricity: {}V, {}Hz, socket types {}\n",
            facts.name,
            facts.voltage,
            facts.frequency,
            facts.plug_types.join(", ")
        ));
        out.push_str(&format!("- {} currency: {}\n", facts.name, facts.currency));
        out.push_str(&format!(
            "- {} drives on the {}\n",
            facts.name,
            match facts.drives_on {
                DrivingSide::Left => "left",
                DrivingSide::Right => "right",
            }
        ));
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

    out.push_str(&format!("\n{}\n", category_scope(category)));

    if !already_listed.is_empty() {
        out.push_str(
            "\nAlready on the packing list (do not repeat these names in any form):\n",
        );
        for name in already_listed {
            out.push_str(&format!("- {name}\n"));
        }
    }

    out.push_str(&format!(
        "\nTask: list at most 5 items in the category \"{}\" that these travellers should pack \
         for this trip. Only this category — refuse anything that belongs elsewhere. Use the \
         facts above rather than your own knowledge. Give each item a one-sentence reason \
         referring to something specific about this trip. Prefer concrete attributes in the \
         name or reason (SPF number for sunscreen, plug type letter and which sockets it fits, \
         voltage notes for adapters, currency code for cash). Mark an item essential only if \
         the trip fails without it.\n",
        category.label()
    ));

    out.push_str(
        "You MUST reason about the supplied mains electricity, currency, driving side, \
         tap water, and climate facts. Do not ignore a fact that is relevant to this category. \
         Name the destination country correctly when citing socket or currency facts.\n",
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
        assert!(p.contains("plug type"));
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
}
