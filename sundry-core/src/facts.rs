use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrivingSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapWater {
    Potable,
    NotPotable,
    Caution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountryFacts {
    pub code: String,
    pub name: String,
    pub plug_types: Vec<String>,
    pub voltage: u32,
    pub frequency: u32,
    pub currency: String,
    pub drives_on: DrivingSide,
    pub tap_water: TapWater,
}

const RAW: &str = include_str!("../data/country-facts.json");

pub struct FactsTable {
    by_code: HashMap<String, CountryFacts>,
}

impl FactsTable {
    pub fn load() -> Self {
        let rows: Vec<CountryFacts> =
            serde_json::from_str(RAW).expect("bundled country-facts.json is malformed");
        let by_code = rows
            .into_iter()
            .map(|f| (f.code.to_uppercase(), f))
            .collect();
        FactsTable { by_code }
    }

    pub fn get(&self, code: &str) -> Option<&CountryFacts> {
        self.by_code.get(&code.to_uppercase())
    }

    pub fn all(&self) -> impl Iterator<Item = &CountryFacts> {
        self.by_code.values()
    }

    pub fn len(&self) -> usize {
        self.by_code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_code.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bundled_table() {
        let table = FactsTable::load();
        assert!(
            table.len() > 150,
            "expected ~200 countries, got {}",
            table.len()
        );
    }

    #[test]
    fn looks_up_japan() {
        let table = FactsTable::load();
        let jp = table.get("JP").expect("JP present");
        assert_eq!(jp.voltage, 100);
        assert_eq!(jp.currency, "JPY");
        assert_eq!(jp.drives_on, DrivingSide::Left);
        assert!(jp.plug_types.contains(&"A".to_string()));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let table = FactsTable::load();
        assert!(table.get("jp").is_some());
    }

    #[test]
    fn unknown_code_returns_none() {
        let table = FactsTable::load();
        assert!(table.get("ZZ").is_none());
    }

    #[test]
    fn every_row_is_plausible() {
        let table = FactsTable::load();
        for facts in table.all() {
            assert_eq!(facts.code.len(), 2, "bad code: {}", facts.code);
            assert!(
                !facts.plug_types.is_empty(),
                "{} has no plug types",
                facts.code
            );
            assert!(
                facts.voltage >= 100 && facts.voltage <= 240,
                "{} has implausible voltage {}",
                facts.code,
                facts.voltage
            );
            assert!(facts.frequency == 50 || facts.frequency == 60);
            assert_eq!(facts.currency.len(), 3, "{} bad currency", facts.code);
        }
    }
}
