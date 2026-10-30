import SwiftUI

/// Editable traveller names in the window toolbar. Commits on Return or focus loss.
struct TravellerNameBar: View {
    @Environment(TripStore.self) private var store
    @State private var drafts: [String] = []
    @FocusState private var focusedIndex: Int?

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "person.2")
                .foregroundStyle(Olive.taupe)
                .help("People packing for this trip")

            ForEach(Array(drafts.enumerated()), id: \.offset) { index, _ in
                TextField("Name", text: binding(at: index))
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 96)
                    .font(.callout)
                    .focused($focusedIndex, equals: index)
                    .onSubmit { commit(at: index) }
                    .accessibilityLabel("Traveller \(index + 1)")
            }
        }
        .onAppear { syncDrafts() }
        .onChange(of: store.travellerNames) { _, _ in
            if focusedIndex == nil { syncDrafts() }
        }
        .onChange(of: focusedIndex) { previous, current in
            if let previous, current != previous {
                commit(at: previous)
            }
        }
    }

    private func binding(at index: Int) -> Binding<String> {
        Binding(
            get: {
                drafts.indices.contains(index) ? drafts[index] : ""
            },
            set: { value in
                guard drafts.indices.contains(index) else { return }
                drafts[index] = value
                // Live-update columns/chips as the name is edited.
                store.setTravellerName(at: index, to: value)
            }
        )
    }

    private func syncDrafts() {
        drafts = store.travellerNames
    }

    private func commit(at index: Int) {
        guard drafts.indices.contains(index) else { return }
        store.setTravellerName(at: index, to: drafts[index])
        // Reflect any trim/rejection from the store.
        if store.travellerNames.indices.contains(index) {
            drafts[index] = store.travellerNames[index]
        }
    }
}
