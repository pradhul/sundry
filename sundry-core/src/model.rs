use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Documents,
    Money,
    Electronics,
    Clothing,
    Footwear,
    Toiletries,
    Health,
    Bags,
    Comfort,
    Activity,
    Misc,
}

impl Category {
    pub const ALL: [Category; 11] = [
        Category::Documents,
        Category::Money,
        Category::Electronics,
        Category::Clothing,
        Category::Footwear,
        Category::Toiletries,
        Category::Health,
        Category::Bags,
        Category::Comfort,
        Category::Activity,
        Category::Misc,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Category::Documents => "Documents",
            Category::Money => "Money",
            Category::Electronics => "Electronics",
            Category::Clothing => "Clothing",
            Category::Footwear => "Footwear",
            Category::Toiletries => "Toiletries",
            Category::Health => "Health & Medication",
            Category::Bags => "Bags & Organisation",
            Category::Comfort => "Comfort & Entertainment",
            Category::Activity => "Activity-Specific",
            Category::Misc => "Miscellaneous",
        }
    }

    pub fn from_label(label: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.label() == label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    Todo,
    Packed,
    NotNeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemSource {
    Model,
    Manual,
    Learned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Owner {
    Shared,
    Traveller(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub quantity: u32,
    pub category: Category,
    pub owner: Owner,
    pub state: ItemState,
    pub source: ItemSource,
    pub rationale: Option<String>,
    pub note: Option<String>,
    pub essential: bool,
}

impl Item {
    pub fn manual(name: &str, category: Category) -> Self {
        Item {
            id: Uuid::new_v4(),
            name: name.to_string(),
            quantity: 1,
            category,
            owner: Owner::Shared,
            state: ItemState::Todo,
            source: ItemSource::Manual,
            rationale: None,
            note: None,
            essential: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purpose {
    Business,
    Leisure,
    Hiking,
    Beach,
    City,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traveller {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TravelWindow {
    Month { year: i32, month: u32 },
    Dates { start: NaiveDate, end: NaiveDate },
}

impl TravelWindow {
    pub fn month(&self) -> u32 {
        match self {
            TravelWindow::Month { month, .. } => *month,
            TravelWindow::Dates { start, .. } => chrono::Datelike::month(start),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destination {
    pub country_code: String,
    pub city: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trip {
    pub id: Uuid,
    pub title: String,
    pub destinations: Vec<Destination>,
    pub window: TravelWindow,
    pub nights: u32,
    pub travellers: Vec<Traveller>,
    pub purposes: Vec<Purpose>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_has_exactly_eleven_values() {
        assert_eq!(Category::ALL.len(), 11);
    }

    #[test]
    fn category_labels_round_trip() {
        for c in Category::ALL {
            assert_eq!(Category::from_label(c.label()), Some(c));
        }
    }

    #[test]
    fn month_window_reports_its_month() {
        let w = TravelWindow::Month {
            year: 2026,
            month: 11,
        };
        assert_eq!(w.month(), 11);
    }

    #[test]
    fn date_window_reports_start_month() {
        let w = TravelWindow::Dates {
            start: NaiveDate::from_ymd_opt(2026, 2, 3).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 2, 11).unwrap(),
        };
        assert_eq!(w.month(), 2);
    }

    #[test]
    fn new_items_start_as_todo() {
        let item = Item::manual("Passport", Category::Documents);
        assert_eq!(item.state, ItemState::Todo);
        assert_eq!(item.source, ItemSource::Manual);
        assert_eq!(item.quantity, 1);
    }
}
