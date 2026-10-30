//! Deterministic placement rules for generated items: purpose fit, home
//! category for well-known items, and whether an item is personal (one per
//! traveller) or shared.

use crate::model::{Category, Purpose};

fn has(n: &str, words: &[&str]) -> bool {
    words.iter().any(|w| n.contains(w))
}

pub fn is_outdoor(purposes: &[Purpose]) -> bool {
    purposes.contains(&Purpose::Hiking)
}

/// Gear that only makes sense on an outdoor / hiking trip.
const OUTDOOR_ONLY: [&str; 14] = [
    "headlamp",
    "head lamp",
    "head torch",
    "flashlight",
    "torch",
    "lantern",
    "tent",
    "sleeping bag",
    "sleeping pad",
    "trekking pole",
    "hiking pole",
    "compass",
    "camping",
    "carabiner",
];

/// Returns the name to keep (possibly renamed), or `None` to drop the item,
/// when it does not fit the trip's purposes.
pub fn fit_purpose(name: &str, purposes: &[Purpose]) -> Option<String> {
    if is_outdoor(purposes) {
        return Some(name.to_string());
    }
    let n = name.to_lowercase();
    if has(&n, &OUTDOOR_ONLY) {
        return None;
    }
    if n.contains("hiking") || n.contains("trekking") || n.contains("trail") {
        // "Thermal hiking boots" -> a city-appropriate equivalent.
        if has(&n, &["boot"]) {
            return Some("Waterproof walking boots".into());
        }
        if has(&n, &["shoe", "sneaker", "trainer"]) {
            return Some("Comfortable walking shoes".into());
        }
        if has(&n, &["sock"]) {
            return Some("Warm socks".into());
        }
        if has(&n, &["backpack", "pack"]) {
            return Some("Daypack".into());
        }
        return None;
    }
    Some(name.to_string())
}

/// The category a well-known item always belongs in, regardless of which
/// category prompt produced it.
pub fn home_category(name: &str) -> Option<Category> {
    let n = name.to_lowercase();
    if has(
        &n,
        &[
            "sunscreen",
            "sun cream",
            "suncream",
            "sunblock",
            "lip balm",
            "moisturi",
            "toothbrush",
            "toothpaste",
            "deodorant",
            "shampoo",
            "conditioner",
            "body wash",
            "razor",
        ],
    ) {
        return Some(Category::Toiletries);
    }
    if has(
        &n,
        &[
            "insect repellent",
            "mosquito repellent",
            "bug spray",
            "first aid",
            "plaster",
            "band-aid",
            "bandage",
            "paracetamol",
            "ibuprofen",
            "painkiller",
            "pain relie",
            "antihistamine",
            "medication",
            "hand sanitiser",
            "hand sanitizer",
        ],
    ) {
        return Some(Category::Health);
    }
    if has(
        &n,
        &[
            "charger",
            "plug adapter",
            "travel adapter",
            "power bank",
            "usb",
            "cable",
            "power strip",
        ],
    ) {
        return Some(Category::Electronics);
    }
    if has(&n, &["sock"]) {
        return Some(Category::Footwear);
    }
    if has(&n, &["passport", "travel insurance", "boarding pass"]) {
        return Some(Category::Documents);
    }
    None
}

/// Items each traveller needs their own of. Everything else is shared.
pub fn is_personal(name: &str) -> bool {
    let n = name.to_lowercase();
    has(
        &n,
        &[
            "toothbrush",
            "coat",
            "jacket",
            "parka",
            "sweater",
            "jumper",
            "fleece",
            "cardigan",
            "glove",
            "mitten",
            "hat",
            "beanie",
            "scarf",
            "sleepwear",
            "pajama",
            "pyjama",
            "underwear",
            "sock",
            "top",
            "shirt",
            "tee",
            "blouse",
            "trouser",
            "pants",
            "jeans",
            "legging",
            "base layer",
            "shoe",
            "boot",
            "sneaker",
            "sandal",
            "passport",
            "water bottle",
            "eye mask",
            "earplug",
            "ear plug",
            "neck pillow",
            "travel pillow",
            "wallet",
            "daypack",
            "sleep mask",
        ],
    ) && !has(&n, &["laptop", "shoe bag", "boot bag"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_trip_drops_or_renames_outdoor_gear() {
        let city = [Purpose::Leisure, Purpose::City];
        assert_eq!(fit_purpose("Headlamp", &city), None);
        assert_eq!(fit_purpose("Portable lantern", &city), None);
        assert_eq!(
            fit_purpose("Thermal hiking boots", &city).as_deref(),
            Some("Waterproof walking boots")
        );
        assert_eq!(
            fit_purpose("Rain jacket", &city).as_deref(),
            Some("Rain jacket")
        );
    }

    #[test]
    fn hiking_trip_keeps_outdoor_gear() {
        let hike = [Purpose::Hiking];
        assert_eq!(fit_purpose("Headlamp", &hike).as_deref(), Some("Headlamp"));
        assert_eq!(
            fit_purpose("Thermal hiking boots", &hike).as_deref(),
            Some("Thermal hiking boots")
        );
    }

    #[test]
    fn well_known_items_have_a_home_category() {
        assert_eq!(
            home_category("Sunscreen SPF 30"),
            Some(Category::Toiletries)
        );
        assert_eq!(home_category("Lip balm"), Some(Category::Toiletries));
        assert_eq!(home_category("Insect repellent"), Some(Category::Health));
        assert_eq!(home_category("Phone charger"), Some(Category::Electronics));
        assert_eq!(
            home_category("Type A/B plug adapter"),
            Some(Category::Electronics)
        );
        assert_eq!(home_category("Wool socks"), Some(Category::Footwear));
        assert_eq!(home_category("Compact umbrella"), None);
    }

    #[test]
    fn personal_vs_shared() {
        for p in [
            "Toothbrush",
            "Warm coat",
            "Sweater",
            "Gloves",
            "Warm hat",
            "Scarf",
            "Sleepwear",
            "Underwear",
            "Wool socks",
            "Long-sleeve tops",
            "Trousers",
            "Comfortable walking shoes",
            "Passport",
            "Reusable water bottle",
            "Eye mask",
        ] {
            assert!(is_personal(p), "{p} should be personal");
        }
        for s in [
            "Compact umbrella",
            "Type A/B plug adapter",
            "Power bank",
            "First aid kit",
            "Laundry bag",
            "Toothpaste",
            "Shampoo",
        ] {
            assert!(!is_personal(s), "{s} should be shared");
        }
    }
}
