import SwiftUI

struct TripSidebar: View {
    @Environment(TripStore.self) private var store

    private var upcoming: [TripSummaryDto] {
        store.trips.filter { isUpcoming($0) }.sorted(by: chronological)
    }

    private var past: [TripSummaryDto] {
        store.trips.filter { !isUpcoming($0) }.sorted(by: chronological).reversed()
    }

    var body: some View {
        List(selection: Binding(
            get: { store.selectedTripId },
            set: { store.select($0) }
        )) {
            if !upcoming.isEmpty {
                Section("Upcoming") {
                    ForEach(upcoming, id: \.id) { row($0) }
                }
            }
            if !past.isEmpty {
                Section("Past") {
                    ForEach(past, id: \.id) { row($0) }
                }
            }
        }
        .navigationTitle("Sundry")
        .toolbar {
            Button {
                store.isCreatingTrip = true
            } label: {
                Label("New Trip", systemImage: "plus")
            }
        }
        .overlay {
            if store.trips.isEmpty {
                ContentUnavailableView(
                    "No trips yet",
                    systemImage: "airplane.departure",
                    description: Text("Add a trip to get a packing list.")
                )
            }
        }
    }

    private func row(_ trip: TripSummaryDto) -> some View {
        VStack(alignment: .leading) {
            Text(trip.title)
            Text("\(monthName(trip.month)) \(String(trip.year))")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .tag(trip.id)
    }

    private func isUpcoming(_ trip: TripSummaryDto) -> Bool {
        let now = Calendar.current.dateComponents([.year, .month], from: .now)
        guard let y = now.year, let m = now.month else { return true }
        return (Int(trip.year), Int(trip.month)) >= (y, m)
    }

    private func chronological(_ a: TripSummaryDto, _ b: TripSummaryDto) -> Bool {
        (a.year, a.month) < (b.year, b.month)
    }
}

func monthName(_ month: UInt32) -> String {
    let names = Calendar.current.monthSymbols
    let index = Int(month) - 1
    return names.indices.contains(index) ? names[index] : "?"
}
