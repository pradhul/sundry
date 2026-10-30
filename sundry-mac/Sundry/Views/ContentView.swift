import SwiftUI

struct ContentView: View {
    @Environment(TripStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var selectedItem: ItemDto?

    var body: some View {
        NavigationSplitView {
            TripSidebar()
        } detail: {
            if store.selectedTripId == nil {
                ContentUnavailableView(
                    "No trip selected",
                    systemImage: "suitcase",
                    description: Text("Choose a trip, or press \u{2318}N to plan a new one.")
                )
                .foregroundStyle(Olive.ink)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Olive.paper)
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            } else {
                ChecklistView(selectedItem: $selectedItem)
                    .transition(.opacity.combined(with: .move(edge: .trailing)))
            }
        }
        .animation(Olive.spring(reduceMotion), value: store.selectedTripId)
        .sheet(isPresented: Binding(
            get: { store.isCreatingTrip },
            set: { store.isCreatingTrip = $0 }
        )) {
            NewTripSheet()
        }
        .sheet(isPresented: Binding(
            get: { store.isAddingItem },
            set: { store.isAddingItem = $0 }
        )) {
            AddItemSheet()
        }
        .confirmationDialog(
            deleteTitle,
            isPresented: Binding(
                get: { store.tripPendingDelete != nil },
                set: { if !$0 { store.tripPendingDelete = nil } }
            ),
            titleVisibility: .visible
        ) {
            if let trip = store.tripPendingDelete {
                Button("Delete Trip", role: .destructive) {
                    store.deleteTrip(trip)
                }
            }
            Button("Cancel", role: .cancel) {
                store.tripPendingDelete = nil
            }
        } message: {
            Text("This removes the trip and its packing list from this Mac.")
        }
        .oliveJournal()
    }

    private var deleteTitle: String {
        if let title = store.tripPendingDelete?.title {
            return "Delete “\(title)”?"
        }
        return "Delete trip?"
    }
}
