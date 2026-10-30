import Foundation
import Observation
import SwiftUI
import UniformTypeIdentifiers

extension ItemDto: Identifiable {}

enum GenerationState: Equatable {
    case idle
    case running(completed: Int, total: Int)
    case finished(climateGrounded: Bool, failures: [String])
    case failed(String)
}

@Observable
@MainActor
final class TripStore {
    var trips: [TripSummaryDto] = []
    var items: [ItemDto] = []
    var selectedTripId: String?
    var tripFacts: TripFactsDto?
    var isCreatingTrip = false
    var isAddingItem = false
    var generationState: GenerationState = .idle
    var orderedCategories: [String] = []
    var statusMessage: String?
    var tripPendingDelete: TripSummaryDto?
    /// The selected trip's travellers (columns, chips, toolbar). Falls back to
    /// the app defaults when no trip is selected.
    var travellerNames: [String] = Travellers.load()
    /// App-level defaults copied into new trips.
    var defaultTravellers: [String] = Travellers.load()

    private let core: SundryCore
    private let inboxWatcher = TripInboxWatcher()
    private var importedPaths = Set<String>()

    init() {
        let root = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Sundry", isDirectory: true)
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)

        core = SundryCore(
            storageRoot: root.path,
            climate: OpenMeteoClimateProvider(),
            llm: FoundationModelsBackend()
        )
        orderedCategories = categoryOrder()
        refresh()
        migrateDefaultTravellersIfNeeded()
        startInboxWatch()
    }

    /// Installs from before per-trip travellers never saved defaults (the old
    /// code fell back to a built-in pair). Adopt the travellers of the most
    /// recent existing trip so new trips keep looking the same; a fresh install
    /// with no trips keeps the macOS-user default.
    private func migrateDefaultTravellersIfNeeded() {
        guard !Travellers.hasSaved else { return }
        for trip in trips.reversed() {
            if let names = try? core.loadTravellers(tripId: trip.id), !names.isEmpty {
                Travellers.save(names)
                defaultTravellers = names
                if selectedTripId == nil { travellerNames = names }
                return
            }
        }
    }

    func setTravellerName(at index: Int, to raw: String) {
        guard travellerNames.indices.contains(index) else { return }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard travellerNames[index] != trimmed else { return }

        travellerNames[index] = trimmed
        applyTravellers()
    }

    func addTraveller() {
        travellerNames.append(Travellers.placeholderName(forIndex: travellerNames.count))
        applyTravellers()
    }

    func removeTraveller(at index: Int) {
        guard travellerNames.count > 1, travellerNames.indices.contains(index) else { return }
        if let id = selectedTripId {
            // Id-preserving removal in the core so other travellers keep their packed items.
            try? core.removeTraveller(tripId: id, index: UInt32(index))
            travellerNames = (try? core.loadTravellers(tripId: id)) ?? travellerNames
        } else {
            travellerNames.remove(at: index)
        }
        saveDefaults(travellerNames)
        refresh()
    }

    func saveDefaults(_ names: [String]) {
        Travellers.save(names)
        defaultTravellers = Travellers.load()
    }

    /// Writes the edited names to the selected trip (by position) and makes
    /// them the defaults for the next new trip.
    private func applyTravellers() {
        if let id = selectedTripId {
            try? core.ensureTravellers(tripId: id, names: travellerNames)
        }
        saveDefaults(travellerNames)
        refresh()
    }

    /// Trips keep their own travellers. Only a legacy trip with none gets the defaults.
    private func loadTravellers(for id: String) {
        let stored = (try? core.loadTravellers(tripId: id)) ?? []
        if stored.isEmpty {
            try? core.ensureTravellers(tripId: id, names: defaultTravellers)
            travellerNames = defaultTravellers
        } else {
            travellerNames = stored
        }
    }

    func refresh() {
        trips = (try? core.listTrips()) ?? []
        if let id = selectedTripId {
            items = (try? core.loadItems(tripId: id)) ?? []
            tripFacts = try? core.loadTripFacts(tripId: id)
        } else {
            items = []
            tripFacts = nil
        }
    }

    func select(_ id: String?) {
        if selectedTripId != id {
            generationState = .idle
            statusMessage = nil
        }
        selectedTripId = id
        if let id {
            loadTravellers(for: id)
            _ = try? core.dedupeTrip(tripId: id)
        } else {
            travellerNames = defaultTravellers
        }
        refresh()
    }

    func createTrip(
        title: String,
        city: String,
        countryCode: String,
        year: Int32,
        month: UInt32,
        nights: UInt32,
        purposes: [String],
        travellers: [String]
    ) throws {
        let names = Travellers.sanitise(travellers)
        let chosen = names.isEmpty ? defaultTravellers : names
        saveDefaults(chosen)
        let id = try core.createTrip(
            title: title,
            city: city,
            countryCode: countryCode,
            year: year,
            month: month,
            nights: nights,
            travellerNames: chosen,
            purposes: purposes
        )
        isCreatingTrip = false
        select(id)
        Task { await generate() }
    }

    func deleteTrip(_ trip: TripSummaryDto) {
        do {
            try core.deleteTrip(tripId: trip.id)
            if selectedTripId == trip.id {
                selectedTripId = nil
                generationState = .idle
            }
            tripPendingDelete = nil
            refresh()
            statusMessage = "Deleted “\(trip.title)”."
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    func setItemState(_ item: ItemDto, _ state: ItemStateDto) {
        guard let tripId = selectedTripId else { return }
        try? core.setItemState(tripId: tripId, itemId: item.id, state: state)
        refresh()
    }

    func packItem(_ item: ItemDto, for travellerName: String) {
        guard let tripId = selectedTripId else { return }
        try? core.setItemPackedFor(
            tripId: tripId,
            itemId: item.id,
            travellerName: travellerName
        )
        refresh()
    }

    func addManualItem(name: String, category: String, quantity: UInt32, essential: Bool) throws {
        guard let tripId = selectedTripId else { return }
        _ = try core.addManualItem(
            tripId: tripId,
            name: name,
            category: category,
            quantity: quantity,
            essential: essential
        )
        isAddingItem = false
        refresh()
        statusMessage = "Added “\(name.trimmingCharacters(in: .whitespaces))”"
    }

    /// One-tap AirDrop — no save dialog; the other Mac's open Sundry picks it up.
    func shareViaAirDrop() {
        guard let tripId = selectedTripId,
              let trip = trips.first(where: { $0.id == tripId }),
              let data = try? core.exportTrip(tripId: tripId)
        else {
            statusMessage = "Couldn’t share this trip."
            return
        }
        do {
            try TripSyncShare.airDrop(data: data, suggestedName: trip.title)
            statusMessage = "AirDrop open — pick the other Mac. Keep Sundry open on both sides."
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    func importTrip(from url: URL) {
        let path = url.standardizedFileURL.path
        guard !importedPaths.contains(path) else { return }

        do {
            let accessed = url.startAccessingSecurityScopedResource()
            defer {
                if accessed { url.stopAccessingSecurityScopedResource() }
            }
            let data = try Data(contentsOf: url)
            let id = try core.importTrip(bytes: data)
            _ = try? core.dedupeTrip(tripId: id)
            importedPaths.insert(path)
            select(id)
            statusMessage = "Synced from shared trip."

            // Prefer not leaving the AirDrop file in Downloads once we've absorbed it.
            if url.path.contains("/Downloads/") {
                try? FileManager.default.removeItem(at: url)
            }
        } catch {
            statusMessage = error.localizedDescription
        }
    }

    func handleIncomingURL(_ url: URL) {
        guard url.pathExtension.lowercased() == "sundry" else { return }
        importTrip(from: url)
    }

    func startInboxWatch() {
        inboxWatcher.start { [weak self] url in
            self?.importTrip(from: url)
        }
    }

    func generate() async {
        guard let tripId = selectedTripId else { return }
        _ = try? core.dedupeTrip(tripId: tripId)
        let total = orderedCategories.count
        generationState = .running(completed: 0, total: total)
        do {
            let outcome = try await core.generate(tripId: tripId)
            let failures = outcome.perCategory.compactMap { c in
                c.error.map { "\(c.category): \($0)" }
            }
            await MainActor.run {
                refresh()
                generationState = .finished(
                    climateGrounded: outcome.climateGrounded,
                    failures: failures
                )
            }
        } catch {
            await MainActor.run {
                refresh()
                generationState = .failed(error.localizedDescription)
            }
        }
    }
}

extension UTType {
    static var sundryTrip: UTType {
        UTType(exportedAs: "com.pradhul.sundry.trip")
    }
}
