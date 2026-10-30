//! Deterministic clean-up of model-generated item names.
//!
//! The on-device model tends to staple grounding facts (voltage, plug types,
//! currency codes, driving side, tap water, SPF) onto unrelated product names
//! and to emit near-duplicates. The Rust core is the source of truth, so we
//! clean or reject those names here instead of trusting the prompt alone.

use crate::model::Category;

const CONNECTORS: [&str; 7] = [" with ", " for ", " in ", " (", ", ", " - ", " featuring "];

/// Words that only make sense as leaked grounding, removed when salvaging a
/// name whose leading modifier was a leaked fact.
const FILLER: [&str; 9] = [
    "awareness",
    "compatible",
    "compatibility",
    "rated",
    "safe",
    "safety",
    "indicator",
    "indicators",
    "friendly",
];

/// Common ISO-4217 codes plus any all-caps three-letter token that is not a
/// well-known product acronym.
const PRODUCT_ACRONYMS: [&str; 12] = [
    "USB", "SIM", "SPF", "LED", "UPF", "GPS", "DSL", "RFID", "ID", "PDF", "AAA", "UV",
];

#[derive(Clone, Copy)]
enum Leak {
    Electrical,
    Currency,
    Driving,
    TapWater,
    Spf,
}

fn token_is_voltage(tok: &str) -> bool {
    let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let lower = t.to_ascii_lowercase();
    for suffix in ["v", "hz", "volt", "volts"] {
        if let Some(num) = lower.strip_suffix(suffix) {
            if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

fn leaks_in(name: &str) -> Vec<Leak> {
    let lower = name.to_lowercase();
    let mut out = Vec::new();
    let tokens: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c == '-' || c == '/' || c == '(' || c == ')')
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.iter().any(|t| token_is_voltage(t))
        || lower.contains("plug")
        || lower.contains("socket")
        || lower.contains("voltage")
    {
        out.push(Leak::Electrical);
    }
    if tokens.iter().any(|t| {
        let t = t.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        t.len() == 3 && t.chars().all(|c| c.is_ascii_uppercase()) && !PRODUCT_ACRONYMS.contains(&t)
    }) {
        out.push(Leak::Currency);
    }
    if lower.contains("driving")
        || lower.contains("left-side")
        || lower.contains("right-side")
        || lower.contains("left side")
        || lower.contains("right side")
    {
        out.push(Leak::Driving);
    }
    if lower.contains("tap water") || lower.contains("tap-water") || lower.contains("potable") {
        out.push(Leak::TapWater);
    }
    if lower.contains("spf") {
        out.push(Leak::Spf);
    }
    out
}

fn is_sun_item(lower: &str) -> bool {
    [
        "sunscreen",
        "sun cream",
        "suncream",
        "sunblock",
        "lip balm",
        "lotion",
        "moisturi",
    ]
    .iter()
    .any(|w| lower.contains(w))
}

fn leak_allowed(category: Category, leak: Leak, name: &str) -> bool {
    let lower = name.to_lowercase();
    match leak {
        Leak::Electrical => category == Category::Electronics,
        Leak::Currency => matches!(category, Category::Money | Category::Documents),
        Leak::Driving => {
            category == Category::Documents && (lower.contains("licen") || lower.contains("permit"))
        }
        Leak::TapWater => false,
        Leak::Spf => is_sun_item(&lower),
    }
}

fn has_forbidden_leak(category: Category, name: &str) -> bool {
    leaks_in(name)
        .into_iter()
        .any(|l| !leak_allowed(category, l, name))
}

fn tidy(name: &str) -> String {
    let collapsed = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|c: char| c == ',' || c == '-' || c == '(' || c == ' ');
    // Leave brand-style casing alone ("iPhone charger").
    if trimmed.chars().nth(1).is_some_and(|c| c.is_uppercase()) {
        return trimmed.to_string();
    }
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Returns a plain product name, or `None` when the name is nothing but leaked
/// grounding and should be dropped.
pub fn clean_name(category: Category, name: &str) -> Option<String> {
    let name = tidy(name);
    if name.is_empty() {
        return None;
    }
    if !has_forbidden_leak(category, &name) {
        return Some(name);
    }

    // 1. Cut a trailing qualifier clause ("Deodorant with A and B plug ...").
    let lower = name.to_lowercase();
    let cut = CONNECTORS.iter().filter_map(|c| lower.find(c)).min();
    if let Some(idx) = cut {
        let head = tidy(&name[..idx]);
        if !head.is_empty() && !has_forbidden_leak(category, &head) {
            return Some(head);
        }
    }

    // 2. Strip leaked leading modifiers ("SPF-rated thermal base layers").
    let base = match cut {
        Some(idx) => name[..idx].to_string(),
        None => name.clone(),
    };
    let kept: Vec<&str> = base
        .split_whitespace()
        .filter(|w| {
            let parts: Vec<&str> = w.split('-').collect();
            let leaked = has_forbidden_leak(category, w)
                || parts.iter().any(|p| {
                    let p = p.to_lowercase();
                    p == "driving" || p == "left" || p == "right" || p == "tap" || p == "water"
                });
            let filler = parts
                .iter()
                .all(|p| FILLER.contains(&p.to_lowercase().as_str()));
            !leaked && !filler
        })
        .collect();
    let salvaged = tidy(&kept.join(" "));
    if salvaged.is_empty() || has_forbidden_leak(category, &salvaged) {
        return None;
    }
    Some(salvaged)
}

const DUP_STOPWORDS: [&str; 22] = [
    "a",
    "an",
    "the",
    "of",
    "and",
    "for",
    "with",
    "pair",
    "pairs",
    "set",
    "lightweight",
    "comfortable",
    "compact",
    "portable",
    "travel",
    "layered",
    "warm",
    "extra",
    "spare",
    "small",
    "large",
    "quality",
];

fn dup_key(name: &str) -> Vec<String> {
    let mut words: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !DUP_STOPWORDS.contains(w))
        .map(|w| {
            if w.len() > 3
                && w.ends_with("es")
                && (w.ends_with("shes") || w.ends_with("ches") || w.ends_with("sses"))
            {
                w[..w.len() - 2].to_string()
            } else if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") {
                w[..w.len() - 1].to_string()
            } else {
                w.to_string()
            }
        })
        .collect();
    words.sort();
    words.dedup();
    words
}

/// True when two names describe the same thing ("SPF-rated thermal base
/// layers" vs "Thermal base layer").
pub fn is_near_duplicate(a: &str, b: &str) -> bool {
    let ka = dup_key(a);
    let kb = dup_key(b);
    if ka.is_empty() || kb.is_empty() {
        return false;
    }
    if ka == kb {
        return true;
    }
    let inter = ka.iter().filter(|w| kb.contains(w)).count();
    let smaller = ka.len().min(kb.len());
    let union = ka.len() + kb.len() - inter;
    (smaller >= 2 && inter == smaller) || (inter as f64 / union as f64) >= 0.75
}

#[cfg(test)]
mod tests {
    use super::*;
    use Category::*;

    fn c(cat: Category, n: &str) -> Option<String> {
        clean_name(cat, n)
    }

    #[test]
    fn cleans_real_kyoto_examples() {
        assert_eq!(
            c(Money, "JPY cash in 100V bills").as_deref(),
            Some("JPY cash")
        );
        assert_eq!(
            c(
                Clothing,
                "Rainwear with waterproof A and B plug compatibility"
            )
            .as_deref(),
            Some("Rainwear")
        );
        assert_eq!(
            c(
                Toiletries,
                "Deodorant with A and B plug-compatible packaging"
            )
            .as_deref(),
            Some("Deodorant")
        );
        assert_eq!(
            c(Toiletries, "Toilet paper (packaged for 100V use)").as_deref(),
            Some("Toilet paper")
        );
        assert_eq!(
            c(
                Clothing,
                "Lightweight sleepwear with left-side driving awareness design"
            )
            .as_deref(),
            Some("Lightweight sleepwear")
        );
        assert_eq!(
            c(Bags, "Clear mesh bags with tap water safety indicators").as_deref(),
            Some("Clear mesh bags")
        );
        assert_eq!(
            c(
                Clothing,
                "Layered jackets with 100V-compatible charging ports"
            )
            .as_deref(),
            Some("Layered jackets")
        );
        assert_eq!(
            c(Clothing, "SPF-rated thermal base layers").as_deref(),
            Some("Thermal base layers")
        );
        assert_eq!(
            c(Toiletries, "SPF-rated toothbrush").as_deref(),
            Some("Toothbrush")
        );
        assert_eq!(
            c(Footwear, "Left-side driving awareness socks").as_deref(),
            Some("Socks")
        );
    }

    #[test]
    fn keeps_legitimate_fact_bearing_names() {
        assert_eq!(
            c(Electronics, "Type A/B plug adapter (100V)").as_deref(),
            Some("Type A/B plug adapter (100V)")
        );
        assert_eq!(
            c(Toiletries, "SPF 50 sunscreen").as_deref(),
            Some("SPF 50 sunscreen")
        );
        assert_eq!(c(Money, "JPY cash").as_deref(), Some("JPY cash"));
        assert_eq!(
            c(Electronics, "USB-C charger").as_deref(),
            Some("USB-C charger")
        );
        assert_eq!(
            c(Documents, "International driving permit").as_deref(),
            Some("International driving permit")
        );
        assert_eq!(
            c(Clothing, "Warm fleece jacket").as_deref(),
            Some("Warm fleece jacket")
        );
    }

    #[test]
    fn rejects_names_that_are_only_leaks() {
        assert_eq!(c(Clothing, "100V plug"), None);
        assert_eq!(c(Misc, "Left-side driving awareness"), None);
    }

    #[test]
    fn detects_near_duplicates() {
        assert!(is_near_duplicate(
            "Thermal base layers",
            "Thermal base layer"
        ));
        assert!(is_near_duplicate(
            "Thermal base layers",
            "Merino thermal base layer"
        ));
        assert!(is_near_duplicate("Clear mesh bags", "Clear mesh bag"));
        assert!(is_near_duplicate("Layered jackets", "Jacket"));
        assert!(!is_near_duplicate("Phone charger", "Phone"));
        assert!(!is_near_duplicate("Walking shoes", "Socks"));
        assert!(!is_near_duplicate("Rain jacket", "Fleece jacket"));
    }

    #[test]
    fn distinct_basics_survive_dedup_and_cleaning() {
        assert!(!is_near_duplicate("Rain jacket", "Warm winter coat"));
        assert!(!is_near_duplicate("Socks", "Underwear"));
        assert!(!is_near_duplicate("Warm socks", "Thermal base layer"));
        assert!(!is_near_duplicate("Trousers", "Thermal leggings"));
        assert_eq!(
            c(Electronics, "Type A plug adapter").as_deref(),
            Some("Type A plug adapter")
        );
        assert!(crate::category_filter::item_fits_category(
            Electronics,
            "Type A plug adapter"
        ));
        for (cat, n) in [
            (Clothing, "Underwear"),
            (Footwear, "Socks"),
            (Clothing, "Trousers"),
            (Clothing, "Gloves"),
            (Clothing, "Warm hat"),
            (Clothing, "Scarf"),
            (Toiletries, "Toothpaste"),
        ] {
            assert_eq!(c(cat, n).as_deref(), Some(n));
            assert!(
                crate::category_filter::item_fits_category(cat, n),
                "{n} rejected"
            );
        }
    }
}
