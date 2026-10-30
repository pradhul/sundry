import SwiftUI

struct ChecklistView: View {
    @Environment(TripStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Binding var selectedItem: ItemDto?
    @State private var query = ""
    @State private var syncPulse = false

    private var visible: [ItemDto] {
        guard !query.isEmpty else { return store.items }
        return store.items.filter { $0.name.localizedCaseInsensitiveContains(query) }
    }

    private var toPack: [ItemDto] {
        visible.filter { $0.state != .packed }
    }

    private var packedCount: Int {
        store.items.filter { $0.state == .packed }.count
    }

    private var isRunning: Bool {
        if case .running = store.generationState { return true }
        return false
    }

    private var isFailed: Bool {
        if case .failed = store.generationState { return true }
        return false
    }

    private var essentialIds: Set<String> {
        Set(toPack.filter(\.essential).map(\.id))
    }

    private var itemSignature: String {
        store.items.map { "\($0.id):\($0.state):\($0.ownerName ?? "")" }.joined(separator: "|")
    }

    var body: some View {
        VStack(spacing: 0) {
            generationChrome

            if let status = store.statusMessage {
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(Olive.moss)
                        .symbolEffect(.bounce, value: status)
                    Text(status)
                        .font(.callout)
                        .foregroundStyle(Olive.ink)
                    Spacer()
                    Button {
                        withAnimation(Olive.softSpring(reduceMotion)) {
                            store.statusMessage = nil
                        }
                    } label: {
                        Image(systemName: "xmark")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(Olive.taupe)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(Olive.cream)
                .overlay(alignment: .bottom) {
                    Rectangle().fill(Olive.rule).frame(height: 1)
                }
                .transition(.move(edge: .top).combined(with: .opacity))
            }

            if store.items.isEmpty, !isRunning, !isFailed {
                ContentUnavailableView(
                    "No list yet",
                    systemImage: "sparkles",
                    description: Text("Generate a packing list for this trip.")
                )
                .foregroundStyle(Olive.ink)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            } else if !store.items.isEmpty || isRunning {
                HSplitView {
                    packingColumn(
                        title: "To pack",
                        count: toPack.count,
                        empty: "Nothing left to pack",
                        items: toPack,
                        essentialsFirst: true,
                        packedStyle: false
                    )

                    ForEach(Array(store.travellerNames.enumerated()), id: \.offset) { _, name in
                        let packed = visible.filter {
                            $0.state == .packed && $0.ownerName == name
                        }
                        packingColumn(
                            title: "Packed for \(name)",
                            count: packed.count,
                            empty: "Nothing packed yet",
                            items: packed,
                            essentialsFirst: false,
                            packedStyle: true
                        )
                    }

                    if selectedItem != nil {
                        ItemInspector(item: selectedItem)
                            .frame(minWidth: 180, idealWidth: 260, maxWidth: 420)
                    }
                }
                .transition(.opacity)
            }
        }
        .background(Olive.paper)
        .searchable(text: $query, prompt: "Find an item")
        .navigationTitle(store.trips.first { $0.id == store.selectedTripId }?.title ?? "Trip")
        .toolbarBackground(Olive.paper, for: .windowToolbar)
        .toolbar {
            ToolbarItemGroup(placement: .navigation) {
                TravellerNameBar()
            }
            ToolbarItem(placement: .status) {
                if case .running = store.generationState {
                    HStack(spacing: 6) {
                        ProgressView().controlSize(.small)
                        Text("Building your list…").foregroundStyle(Olive.taupe)
                    }
                } else if !store.items.isEmpty {
                    Text("\(packedCount) of \(store.items.count) packed")
                        .monospacedDigit()
                        .foregroundStyle(Olive.taupe)
                        .contentTransition(.numericText())
                        .animation(Olive.spring(reduceMotion), value: packedCount)
                }
            }
            ToolbarItemGroup {
                Button {
                    store.isAddingItem = true
                } label: {
                    Label("Add", systemImage: "plus")
                }
                .disabled(store.selectedTripId == nil || isRunning)
                .help("Add an item by hand")

                Button {
                    syncPulse.toggle()
                    store.shareViaAirDrop()
                } label: {
                    Label("Sync", systemImage: "arrow.triangle.2.circlepath")
                        .symbolEffect(.rotate, value: syncPulse)
                }
                .disabled(store.selectedTripId == nil)
                .help("AirDrop this trip. Keep Sundry open on the other Mac to sync automatically.")

                Button {
                    Task { await store.generate() }
                } label: {
                    Label("Regenerate", systemImage: "sparkles")
                }
                .disabled(isRunning)
            }
        }
        .onKeyPress(.space) {
            guard let item = selectedItem, let first = store.travellerNames.first else { return .ignored }
            apply { store.packItem(item, for: first) }
            return .handled
        }
        .onKeyPress(KeyEquivalent("n")) {
            guard let item = selectedItem else { return .ignored }
            apply { store.setItemState(item, .notNeeded) }
            return .handled
        }
        .animation(Olive.softSpring(reduceMotion), value: itemSignature)
        .animation(Olive.spring(reduceMotion), value: store.generationState)
        .animation(Olive.softSpring(reduceMotion), value: store.statusMessage)
    }

    @ViewBuilder
    private func packingColumn(
        title: String,
        count: Int,
        empty: String,
        items: [ItemDto],
        essentialsFirst: Bool,
        packedStyle: Bool
    ) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text(title)
                    .font(.system(.body, design: .serif).weight(.semibold))
                    .foregroundStyle(Olive.ink)
                Text("\(count)")
                    .font(.caption2.weight(.bold).monospacedDigit())
                    .foregroundStyle(Olive.paper)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(count == 0 ? Olive.taupe.opacity(0.7) : Olive.sap)
                    .clipShape(Capsule())
                Spacer()
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(Olive.cream.opacity(0.65))
            .overlay(alignment: .bottom) {
                Rectangle().fill(Olive.rule).frame(height: 1)
            }

            if items.isEmpty {
                VStack(spacing: 10) {
                    Image(systemName: "suitcase")
                        .font(.system(size: 28, weight: .light))
                        .foregroundStyle(Olive.taupe.opacity(0.7))
                    Text(empty)
                        .font(.callout)
                        .foregroundStyle(Olive.taupe)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding()
                .transition(.opacity.combined(with: .scale(scale: 0.97)))
            } else {
                List(selection: $selectedItem) {
                    if essentialsFirst {
                        let essentials = items.filter(\.essential)
                        if !essentials.isEmpty {
                            Section {
                                ForEach(essentials, id: \.id) { item in
                                    row(item, packedStyle: packedStyle)
                                        .tag(item)
                                        .transition(Olive.packTransition)
                                }
                            } header: {
                                sectionHeader("Essentials")
                            }
                        }
                        ForEach(store.orderedCategories, id: \.self) { category in
                            let inCategory = items.filter {
                                $0.category == category && !essentialIds.contains($0.id)
                            }
                            if !inCategory.isEmpty {
                                Section {
                                    ForEach(inCategory, id: \.id) { item in
                                        row(item, packedStyle: packedStyle)
                                            .tag(item)
                                            .transition(Olive.packTransition)
                                    }
                                } header: {
                                    sectionHeader(category)
                                }
                            }
                        }
                    } else {
                        ForEach(items, id: \.id) { item in
                            row(item, packedStyle: packedStyle)
                                .tag(item)
                                .transition(Olive.packTransition)
                        }
                    }
                }
                .scrollContentBackground(.hidden)
                .listRowBackground(Olive.paper)
            }
        }
        .frame(minWidth: 160, idealWidth: 240, maxWidth: .infinity)
        .background(Olive.paper)
    }

    private func sectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.caption2.weight(.semibold))
            .tracking(1.3)
            .textCase(.uppercase)
            .foregroundStyle(Olive.taupe)
    }

    @ViewBuilder
    private var generationChrome: some View {
        switch store.generationState {
        case .running(_, let total):
            VStack(alignment: .leading, spacing: 10) {
                ProgressView()
                    .controlSize(.regular)
                Text("Building your packing list")
                    .font(.system(.title3, design: .serif).weight(.semibold))
                    .foregroundStyle(Olive.ink)
                Text("\(total) categories · first run can take about a minute while Apple Intelligence warms up.")
                    .font(.callout)
                    .foregroundStyle(Olive.taupe)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(20)
            .background(Olive.cream)
            .transition(.move(edge: .top).combined(with: .opacity))
        case .failed(let reason):
            VStack(alignment: .leading, spacing: 10) {
                Text("Couldn’t build this list")
                    .font(.system(.title3, design: .serif).weight(.semibold))
                    .foregroundStyle(Olive.ink)
                Text(reason)
                    .font(.callout)
                    .foregroundStyle(Olive.taupe)
                Button("Retry") {
                    Task { await store.generate() }
                }
                .buttonStyle(OlivePrimaryButtonStyle())
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(20)
            .background(Olive.cream)
            .transition(.move(edge: .top).combined(with: .opacity))
        case .finished(let grounded, let failures):
            VStack(alignment: .leading, spacing: 8) {
                if !grounded {
                    banner(
                        "Generated without climate data",
                        detail: "The weather lookup failed, so this list isn’t tailored to the season. Regenerate when you’re back online.",
                        icon: "cloud.slash"
                    )
                }
                if !failures.isEmpty {
                    banner(
                        "\(failures.count) categories didn’t generate",
                        detail: failures.joined(separator: "\n"),
                        icon: "exclamationmark.triangle"
                    )
                    Button("Retry failed") {
                        Task { await store.generate() }
                    }
                    .buttonStyle(OlivePrimaryButtonStyle())
                    .padding(.leading, 28)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, failures.isEmpty && grounded ? 0 : 12)
            .transition(.opacity)
        case .idle:
            EmptyView()
        }
    }

    private func row(_ item: ItemDto, packedStyle: Bool) -> some View {
        Group {
            if packedStyle {
                packedRow(item)
            } else {
                toPackRow(item)
            }
        }
        .padding(.vertical, 4)
        .listRowBackground(Olive.paper)
        .listRowSeparatorTint(Olive.rule)
    }

    private func toPackRow(_ item: ItemDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Text(item.name)
                    .font(.body.weight(.medium))
                    .foregroundStyle(item.state == .notNeeded ? Olive.taupe : Olive.ink)
                    .strikethrough(item.state == .notNeeded, color: Olive.taupe)
                    .animation(Olive.snappy(reduceMotion), value: item.state)
                if item.quantity > 1 {
                    Text("×\(item.quantity)")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(Olive.taupe)
                }
                if item.essential {
                    Text("Essential")
                        .font(.caption2.weight(.semibold))
                        .tracking(0.3)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .foregroundStyle(Olive.sap)
                        .background(Olive.cream)
                        .clipShape(Capsule())
                }
                Spacer(minLength: 0)
            }

            ItemStateControl(
                state: item.state,
                ownerName: item.ownerName,
                layout: .row
            ) { action in
                switch action {
                case .later:
                    apply { store.setItemState(item, .todo) }
                case .notNeeded:
                    apply { store.setItemState(item, .notNeeded) }
                case .pack(let name):
                    apply { store.packItem(item, for: name) }
                }
            }

            if let rationale = item.rationale, !rationale.isEmpty {
                Text(rationale)
                    .font(.caption)
                    .foregroundStyle(Olive.taupe)
                    .lineLimit(2)
            }
        }
        .frame(minHeight: 44, alignment: .leading)
    }

    private func packedRow(_ item: ItemDto) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(Olive.moss)
                .font(.body)
                .padding(.top, 2)

            VStack(alignment: .leading, spacing: 4) {
                Text(item.name)
                    .font(.body.weight(.semibold))
                    .foregroundStyle(Olive.ink)
                if let rationale = item.rationale, !rationale.isEmpty {
                    Text(rationale)
                        .font(.caption)
                        .foregroundStyle(Olive.taupe)
                        .lineLimit(2)
                }
                HStack(spacing: 6) {
                    Button("Later") {
                        apply { store.setItemState(item, .todo) }
                    }
                    .buttonStyle(.plain)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Olive.accent)

                    ForEach(store.travellerNames.filter { $0 != item.ownerName }, id: \.self) { name in
                        Button(name) {
                            apply { store.packItem(item, for: name) }
                        }
                        .buttonStyle(.plain)
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(Olive.accent)
                    }

                    Button("Not needed") {
                        apply { store.setItemState(item, .notNeeded) }
                    }
                    .buttonStyle(.plain)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Olive.taupe)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(Olive.cream.opacity(0.55))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(Olive.rule, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private func apply(_ work: () -> Void) {
        if let animation = Olive.softSpring(reduceMotion) {
            withAnimation(animation, work)
        } else {
            work()
        }
    }

    private func banner(_ title: String, detail: String, icon: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: icon).foregroundStyle(Olive.accent)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.callout.weight(.medium)).foregroundStyle(Olive.ink)
                Text(detail).font(.caption).foregroundStyle(Olive.taupe)
            }
        }
        .padding(8)
    }
}

enum ItemPackAction {
    case later
    case notNeeded
    case pack(String)
}

enum ItemStateControlLayout {
    /// Full labels in a horizontal row under the item title.
    case row
    /// Full labels stacked vertically (inspector / packed card).
    case stack
}

struct ItemStateControl: View {
    @Environment(TripStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let state: ItemStateDto
    let ownerName: String?
    var layout: ItemStateControlLayout = .stack
    let onChange: (ItemPackAction) -> Void

    var body: some View {
        Group {
            switch layout {
            case .row:
                HStack(spacing: 4) {
                    chip("Later", active: state == .todo) { onChange(.later) }
                    ForEach(Array(store.travellerNames.enumerated()), id: \.offset) { _, name in
                        chip(
                            name,
                            active: state == .packed && ownerName == name,
                            packed: true
                        ) {
                            onChange(.pack(name))
                        }
                    }
                    chip("Not needed", active: state == .notNeeded) {
                        onChange(.notNeeded)
                    }
                    Spacer(minLength: 0)
                }
            case .stack:
                VStack(alignment: .leading, spacing: 3) {
                    chip("Later", active: state == .todo, stretch: true) { onChange(.later) }
                    ForEach(Array(store.travellerNames.enumerated()), id: \.offset) { _, name in
                        chip(
                            name,
                            active: state == .packed && ownerName == name,
                            packed: true,
                            stretch: true
                        ) {
                            onChange(.pack(name))
                        }
                    }
                    chip("Not needed", active: state == .notNeeded, stretch: true) {
                        onChange(.notNeeded)
                    }
                }
            }
        }
    }

    private func chip(
        _ title: String,
        active: Bool,
        packed: Bool = false,
        stretch: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(title)
                .font(.caption2.weight(.semibold))
                .lineLimit(1)
                .frame(
                    maxWidth: stretch ? .infinity : nil,
                    alignment: stretch ? .leading : .center
                )
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .foregroundStyle(active ? Olive.paper : Olive.ink.opacity(0.7))
                .background(active ? (packed ? Olive.moss : Olive.sap) : Olive.cream)
                .overlay(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .strokeBorder(Olive.buttonBorder.opacity(active ? 0 : 1), lineWidth: 1)
                )
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                .scaleEffect(active && !reduceMotion ? 1.02 : 1)
                .animation(Olive.snappy(reduceMotion), value: active)
        }
        .buttonStyle(.plain)
        .help(title)
    }
}
