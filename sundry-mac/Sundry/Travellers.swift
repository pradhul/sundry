import Foundation

enum Travellers {
    static let defaults = ["Appu", "Swathi"]
    private static let storageKey = "sundry.travellerNames"

    static func load() -> [String] {
        guard let saved = UserDefaults.standard.stringArray(forKey: storageKey),
              saved.count >= 2
        else {
            return defaults
        }
        return saved.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .map { $0.isEmpty ? "Traveller" : $0 }
    }

    static func save(_ names: [String]) {
        UserDefaults.standard.set(names, forKey: storageKey)
    }
}
