import Foundation

/// App-level default travellers. New trips copy these; each trip then owns its
/// own traveller list (stored in the trip document), so editing defaults never
/// rewrites existing trips.
enum Travellers {
    private static let storageKey = "sundry.travellerNames"

    /// Fresh install: one traveller named after the macOS user.
    static var freshInstallDefault: [String] {
        let full = NSFullUserName().trimmingCharacters(in: .whitespacesAndNewlines)
        return [full.isEmpty ? "Me" : full]
    }

    static var hasSaved: Bool {
        UserDefaults.standard.stringArray(forKey: storageKey) != nil
    }

    static func load() -> [String] {
        guard let saved = UserDefaults.standard.stringArray(forKey: storageKey) else {
            return freshInstallDefault
        }
        let cleaned = sanitise(saved)
        return cleaned.isEmpty ? freshInstallDefault : cleaned
    }

    static func save(_ names: [String]) {
        let cleaned = sanitise(names)
        guard !cleaned.isEmpty else { return }
        UserDefaults.standard.set(cleaned, forKey: storageKey)
    }

    static func sanitise(_ names: [String]) -> [String] {
        names.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
    }

    static func placeholderName(forIndex index: Int) -> String {
        "Traveller \(index + 1)"
    }
}
