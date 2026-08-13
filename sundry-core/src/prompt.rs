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

pub fn build_prompt(ctx: &GroundedContext, category: Category) -> String {
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

    out.push_str(&format!(
        "\nTask: list the items in the category \"{}\" that these travellers should pack \
         for this trip. Only this category. Use the facts above rather than your own \
         knowledge. Give each item a one-sentence reason referring to something specific \
         about this trip. Mark an item essential only if the trip fails without it.\n",
        category.label()
    ));

    out.push_str(
        "You MUST reason about the supplied mains electricity, currency, driving side, \
         tap water, and climate facts. Do not ignore a fact that is relevant to this category.\n",
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
        let p = build_prompt(&kyoto_context(None), Category::Electronics);
        assert!(p.contains("100V"), "voltage missing from prompt");
        assert!(p.contains("50Hz"), "frequency missing from prompt");
        assert!(p.contains("A, B"), "plug types missing from prompt");
    }

    #[test]
    fn prompt_contains_climate_when_available() {
        let p = build_prompt(
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
        let p = build_prompt(&kyoto_context(None), Category::Clothing);
        assert!(
            p.to_lowercase().contains("climate data is unavailable"),
            "must state climate is unknown rather than omitting it silently"
        );
    }

    #[test]
    fn prompt_names_only_the_requested_category() {
        let p = build_prompt(&kyoto_context(None), Category::Footwear);
        assert!(p.contains("Footwear"));
        assert!(
            !p.contains("Toiletries"),
            "must not mention other categories"
        );
    }

    #[test]
    fn prompt_instructs_the_model_not_to_use_its_own_knowledge() {
        let p = build_prompt(&kyoto_context(None), Category::Electronics);
        assert!(p.contains("do not contradict"));
    }

    #[test]
    fn documents_prompt_forbids_asserting_entry_rules() {
        let p = build_prompt(&kyoto_context(None), Category::Documents);
        assert!(
            p.contains("Do not state visa or entry requirements"),
            "documents prompt must forbid asserting entry rules"
        );
    }
}
