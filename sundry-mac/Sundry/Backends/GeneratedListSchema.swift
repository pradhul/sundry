import FoundationModels

/// Mirrors sundry_core::llm::GeneratedItem. Exists only to drive constrained
/// decoding; Rust remains the authority on shape and performs all validation.
@Generable
struct GeneratedItemSchema: Codable {
    @Guide(description: "Plain product name, e.g. 'Rain jacket' or 'Toothpaste'")
    var name: String
    @Guide(description: "How many to pack, between 1 and 20")
    var quantity: Int
    @Guide(description: "One sentence referring to something specific about this trip")
    var rationale: String
    @Guide(description: "True only if the trip fails without this item")
    var essential: Bool
}

@Generable
struct GeneratedListSchema: Codable {
    @Guide(description: "Between 4 and 8 distinct items", .maximumCount(8))
    var items: [GeneratedItemSchema]
}
