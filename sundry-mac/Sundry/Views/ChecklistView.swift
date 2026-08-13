import SwiftUI

struct ChecklistView: View {
    @Environment(TripStore.self) private var store
    @Binding var selectedItem: ItemDto?
    @State private var query = ""

    private var visible: [ItemDto] {
        guard !query.isEmpty else { return store.items }
        return store.items.filter { $0.name.localizedCaseInsensitiveContains(query) }
    }

    private var packedCount: Int {
        store.items.filter { $0.state == .packed }.count
    }

    var body: some View {
        List(selection: $selectedItem) {
            if case .finished(let grounded, let failures) = store.generationState {
                if !grounded {
                    banner(
                        "Generated without climate data",
                        detail: "The weather lookup failed, so this list isn't tailored to the season. Regenerate when you're back online.",
                        icon: "cloud.slash"
                    )
                }
                if !failures.isEmpty {
                    banner(
                        "\(failures.count) categories didn't generate",
                        detail: failures.joined(separator: "\n"),
                        icon: "exclamationmark.triangle"
                    )
                }
            }

            ForEach(store.categoryOrder, id: \.self) { category in
                let inCategory = visible.filter { $0.category == category }
                if !inCategory.isEmpty {
                    Section(category) {
                        ForEach(inCategory, id: \.id) { item in
                            row(item).tag(item)
                        }
                    }
                }
            }
        }
        .searchable(text: $query, prompt: "Find an item")
        .navigationTitle(store.trips.first { $0.id == store.selectedTripId }?.title ?? "Trip")
        .toolbar {
            ToolbarItem(placement: .status) {
                if case .running = store.generationState {
                    HStack(spacing: 6) {
                        ProgressView().controlSize(.small)
                        Text("Building your list…").foregroundStyle(.secondary)
                    }
                } else if !store.items.isEmpty {
                    Text("\(packedCount) of \(store.items.count) packed")
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                }
            }
            ToolbarItem {
                Button {
                    Task { await store.generate() }
                } label: {
                    Label("Regenerate", systemImage: "arrow.clockwise")
                }
                .disabled(isRunning)
            }
        }
        .overlay {
            if store.items.isEmpty, !isRunning {
                ContentUnavailableView(
                    "No list yet",
                    systemImage: "sparkles",
                    description: Text("Generate a packing list for this trip.")
                )
            }
        }
    }

    private var isRunning: Bool {
        if case .running = store.generationState { return true }
        return false
    }

    private func row(_ item: ItemDto) -> some View {
        Toggle(isOn: Binding(
            get: { item.state == .packed },
            set: { _ in store.toggle(item) }
        )) {
            HStack {
                Text(item.name)
                    .strikethrough(item.state == .packed, color: .secondary)
                if item.quantity > 1 {
                    Text("×\(item.quantity)").foregroundStyle(.secondary).monospacedDigit()
                }
                if item.essential {
                    Image(systemName: "exclamationmark.circle.fill")
                        .foregroundStyle(.orange)
                        .help("Essential")
                }
            }
        }
        .toggleStyle(.checkbox)
    }

    private func banner(_ title: String, detail: String, icon: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: icon).foregroundStyle(.orange)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.callout.weight(.medium))
                Text(detail).font(.caption).foregroundStyle(.secondary)
            }
        }
        .padding(8)
    }
}
