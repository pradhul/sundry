use crate::llm::normalise;
use crate::model::Category;

/// Returns false when the model clearly dumped an item into the wrong category.
pub fn item_fits_category(category: Category, name: &str) -> bool {
    let n = normalise(name);
    match category {
        Category::Documents => {
            looks_like_document(&n)
                && !contains_any(
                    &n,
                    &[
                        "shoe",
                        "sandal",
                        "boot",
                        "sock",
                        "sunscreen",
                        "adapter",
                        "charger",
                        "camera",
                        "clothing",
                        "shirt",
                        "jacket",
                        "raincoat",
                        "hat",
                        "daypack",
                        "guidebook",
                    ],
                )
        }
        Category::Money => {
            looks_like_money(&n)
                && !contains_any(
                    &n,
                    &[
                        "shoe",
                        "sandal",
                        "boot",
                        "camera",
                        "clothing",
                        "shirt",
                        "jacket",
                        "sunscreen",
                        "hat",
                        "daypack",
                        "guidebook",
                        "raincoat",
                        "first aid",
                        "sim card",
                        "adapter",
                        "charger",
                    ],
                )
        }
        Category::Electronics => !contains_any(
            &n,
            &[
                "shoe",
                "sandal",
                "passport",
                "sunscreen",
                "shirt",
                "jacket",
                "sock",
                "wallet",
                "cash",
            ],
        ),
        Category::Clothing => !contains_any(
            &n,
            &[
                "shoe",
                "sandal",
                "boot",
                "passport",
                "adapter",
                "charger",
                "camera",
                "wallet",
                "sunscreen",
            ],
        ),
        Category::Footwear => {
            contains_any(&n, &["shoe", "sandal", "boot", "sock", "flip", "sneaker", "trainer"])
                || !contains_any(
                    &n,
                    &[
                        "passport",
                        "adapter",
                        "charger",
                        "camera",
                        "wallet",
                        "sunscreen",
                        "shirt",
                        "jacket",
                    ],
                )
        }
        Category::Toiletries => !contains_any(
            &n,
            &["shoe", "passport", "adapter", "camera", "wallet", "shirt", "jacket"],
        ),
        Category::Health => !contains_any(
            &n,
            &["shoe", "passport", "adapter", "camera", "wallet", "shirt", "guidebook"],
        ),
        Category::Bags => {
            contains_any(&n, &["bag", "pack", "cube", "luggage", "suitcase", "pouch", "organiser", "organizer"])
                || !contains_any(
                    &n,
                    &["shoe", "passport", "sunscreen", "adapter", "shirt", "wallet"],
                )
        }
        Category::Comfort => !contains_any(
            &n,
            &["shoe", "passport", "adapter", "wallet", "sunscreen", "shirt"],
        ),
        Category::Activity => !contains_any(&n, &["passport", "wallet", "adapter"]),
        Category::Misc => true,
    }
}

fn looks_like_document(n: &str) -> bool {
    contains_any(
        n,
        &[
            "passport",
            "visa",
            "ticket",
            "insurance",
            "boarding",
            "reservation",
            "itinerary",
            "permit",
            "license",
            "licence",
            "document",
            "id card",
            "identity",
            "photocopy",
            "copy of",
            "printout",
        ],
    )
}

fn looks_like_money(n: &str) -> bool {
    contains_any(
        n,
        &[
            "wallet",
            "cash",
            "card",
            "currency",
            "money",
            "atm",
            "coin",
            "forex",
            "exchange",
            "belt",
            "debit",
            "credit",
        ],
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shoes_rejected_from_documents_and_money() {
        assert!(!item_fits_category(
            Category::Documents,
            "Comfortable walking shoes"
        ));
        assert!(!item_fits_category(
            Category::Money,
            "Comfortable walking shoes"
        ));
    }

    #[test]
    fn wallet_accepted_in_money() {
        assert!(item_fits_category(Category::Money, "Local currency cash"));
        assert!(item_fits_category(Category::Money, "Credit card"));
        assert!(item_fits_category(Category::Money, "Money belt"));
    }

    #[test]
    fn passport_accepted_in_documents() {
        assert!(item_fits_category(Category::Documents, "Passport"));
        assert!(item_fits_category(Category::Documents, "Travel insurance"));
    }

    #[test]
    fn sunscreen_rejected_from_documents() {
        assert!(!item_fits_category(Category::Documents, "Sunscreen"));
    }

    #[test]
    fn adapter_accepted_in_electronics() {
        assert!(item_fits_category(
            Category::Electronics,
            "Type F plug adapter"
        ));
        assert!(!item_fits_category(
            Category::Documents,
            "Type F plug adapter"
        ));
    }
}
