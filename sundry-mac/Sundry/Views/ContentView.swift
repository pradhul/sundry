import SwiftUI

struct ContentView: View {
    @Environment(TripStore.self) private var store
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
            } else {
                ChecklistView(selectedItem: $selectedItem)
            }
        }
        .inspector(isPresented: .constant(selectedItem != nil)) {
            ItemInspector(item: selectedItem)
        }
        .sheet(isPresented: Binding(
            get: { store.isCreatingTrip },
            set: { store.isCreatingTrip = $0 }
        )) {
            NewTripSheet()
        }
    }
}
