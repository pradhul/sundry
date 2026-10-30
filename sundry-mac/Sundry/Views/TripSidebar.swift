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
        VStack(alignment: .leading, spacing: 0) {
            Text("Sundry")
                .font(.system(.title2, design: .serif).weight(.bold))
                .tracking(-0.4)
                .foregroundStyle(Olive.paper)
                .padding(.horizontal, 16)
                .padding(.top, 18)
                .padding(.bottom, 14)

            List(selection: Binding(
                get: { store.selectedTripId },
                set: { store.select($0) }
            )) {
                if !upcoming.isEmpty {
                    Section {
                        ForEach(upcoming, id: \.id) { row($0) }
                    } header: {
                        Text("Upcoming trips")
                            .font(.caption2.weight(.semibold))
                            .tracking(1.4)
                            .textCase(.uppercase)
                            .foregroundStyle(Olive.paper.opacity(0.5))
                    }
                }
                if !past.isEmpty {
                    Section {
                        ForEach(past, id: \.id) { row($0) }
                    } header: {
                        Text("Past trips")
                            .font(.caption2.weight(.semibold))
                            .tracking(1.4)
                            .textCase(.uppercase)
                            .foregroundStyle(Olive.paper.opacity(0.5))
                    }
                }
            }
            .listStyle(.sidebar)
            .scrollContentBackground(.hidden)
        }
        .background(Olive.sap)
        .foregroundStyle(Olive.paper)
        .toolbarBackground(Olive.sap, for: .windowToolbar)
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
                .foregroundStyle(Olive.paper)
            }
        }
    }

    private func row(_ trip: TripSummaryDto) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(trip.title)
                .font(.body.weight(.semibold))
                .tracking(0.1)
            Text("\(monthName(trip.month)) \(String(trip.year))")
                .font(.caption)
                .foregroundStyle(Olive.paper.opacity(0.65))
        }
        .tag(trip.id)
        .listRowBackground(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(store.selectedTripId == trip.id ? Olive.paper.opacity(0.14) : Color.clear)
                .padding(.horizontal, 4)
        )
        .padding(.vertical, 4)
        .contextMenu {
            Button("Delete Trip…", role: .destructive) {
                store.tripPendingDelete = trip
            }
        }
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
