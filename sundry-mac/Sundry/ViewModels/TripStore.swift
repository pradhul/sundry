import Foundation
import Observation

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
    var isCreatingTrip = false
    var generationState: GenerationState = .idle
    var orderedCategories: [String] = []

    private let core: SundryCore

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
    }

    func refresh() {
        trips = (try? core.listTrips()) ?? []
        if let id = selectedTripId {
            items = (try? core.loadItems(tripId: id)) ?? []
        } else {
            items = []
        }
    }

    func select(_ id: String?) {
        selectedTripId = id
        generationState = .idle
        refresh()
    }

    func createTrip(
        title: String,
        city: String,
        countryCode: String,
        year: Int32,
        month: UInt32,
        nights: UInt32,
        travellers: [String],
        purposes: [String]
    ) {
        guard let id = try? core.createTrip(
            title: title,
            city: city,
            countryCode: countryCode,
            year: year,
            month: month,
            nights: nights,
            travellerNames: travellers,
            purposes: purposes
        ) else { return }
        isCreatingTrip = false
        select(id)
        Task { await generate() }
    }

    func toggle(_ item: ItemDto) {
        guard let tripId = selectedTripId else { return }
        let next: ItemStateDto = item.state == .packed ? .todo : .packed
        try? core.setItemState(tripId: tripId, itemId: item.id, state: next)
        refresh()
    }

    func generate() async {
        guard let tripId = selectedTripId else { return }
        let total = orderedCategories.count
        generationState = .running(completed: 0, total: total)
        do {
            let outcome = try await core.generate(tripId: tripId)
            let failures = outcome.perCategory.compactMap { c in
                c.error.map { "\(c.category): \($0)" }
            }
            generationState = .finished(
                climateGrounded: outcome.climateGrounded,
                failures: failures
            )
        } catch {
            generationState = .failed(error.localizedDescription)
        }
        refresh()
    }
}
