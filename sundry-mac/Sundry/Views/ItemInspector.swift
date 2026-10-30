import SwiftUI

struct ItemInspector: View {
    @Environment(TripStore.self) private var store
    let item: ItemDto?

    var body: some View {
        Group {
            if let item {
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        Text(item.name)
                            .font(.system(.title3, design: .serif).weight(.semibold))
                            .foregroundStyle(Olive.ink)
                            .fixedSize(horizontal: false, vertical: true)

                        Text(stateLabel(item))
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .foregroundStyle(item.state == .packed ? Olive.moss : Olive.ink.opacity(0.7))
                            .background(Olive.cream)
                            .clipShape(Capsule())

                        FlowTags(
                            tags: {
                                var t: [String] = []
                                if item.essential { t.append("Essential") }
                                t.append(item.category)
                                if item.quantity > 1 { t.append("×\(item.quantity)") }
                                return t
                            }()
                        )

                        if let rationale = item.rationale, !rationale.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Notes")
                                    .font(.caption.weight(.semibold))
                                    .tracking(0.8)
                                    .textCase(.uppercase)
                                    .foregroundStyle(Olive.taupe)
                                Text(rationale)
                                    .font(.callout)
                                    .foregroundStyle(Olive.ink)
                                    .fixedSize(horizontal: false, vertical: true)
                                    .padding(10)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .background(Olive.cream.opacity(0.7))
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                                            .strokeBorder(Olive.rule, lineWidth: 1)
                                    )
                                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                            }
                        }

                        if let facts = relevantFacts(for: item) {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Destination")
                                    .font(.caption.weight(.semibold))
                                    .tracking(0.8)
                                    .textCase(.uppercase)
                                    .foregroundStyle(Olive.taupe)
                                ForEach(facts, id: \.self) { line in
                                    Text(line)
                                        .font(.callout)
                                        .foregroundStyle(Olive.ink)
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                            }
                        }
                    }
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                ContentUnavailableView("No item selected", systemImage: "sidebar.right")
                    .foregroundStyle(Olive.taupe)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Olive.paper)
        .foregroundStyle(Olive.ink)
    }

    private func stateLabel(_ item: ItemDto) -> String {
        switch item.state {
        case .todo: return "Later"
        case .packed:
            if let owner = item.ownerName {
                return "Packed for \(owner)"
            }
            return "Packed"
        case .notNeeded: return "Not needed"
        }
    }

    private func relevantFacts(for item: ItemDto) -> [String]? {
        guard let trip = store.tripFacts else { return nil }
        let name = item.name.lowercased()
        let category = item.category.lowercased()
        var lines: [String] = []

        let electrical = name.contains("adapter")
            || name.contains("plug")
            || name.contains("charger")
            || category.contains("electronics")
        let money = name.contains("cash")
            || name.contains("wallet")
            || name.contains("currency")
            || name.contains("card")
            || category == "money"

        if electrical || money {
            if let country = trip.countryName {
                lines.append("\(trip.city), \(country)")
            } else if !trip.city.isEmpty {
                lines.append(trip.city)
            }
        }
        if electrical {
            if !trip.plugTypes.isEmpty {
                let plugs = trip.plugTypes.joined(separator: ", ")
                lines.append("Sockets: type \(plugs)")
            }
            if let voltage = trip.voltage, let frequency = trip.frequency {
                lines.append("Mains: \(voltage)V / \(frequency)Hz")
            }
        }
        if money, let currency = trip.currency {
            lines.append("Currency: \(currency)")
        }
        return lines.isEmpty ? nil : lines
    }
}

/// Simple wrapping tag row that shrinks with the inspector width.
private struct FlowTags: View {
    let tags: [String]

    var body: some View {
        // LazyVGrid with adaptive columns flexes instead of overflowing.
        LazyVGrid(
            columns: [GridItem(.adaptive(minimum: 64), spacing: 6, alignment: .leading)],
            alignment: .leading,
            spacing: 6
        ) {
            ForEach(tags, id: \.self) { text in
                Text(text)
                    .font(.caption2.weight(.semibold))
                    .tracking(0.3)
                    .lineLimit(1)
                    .minimumScaleFactor(0.85)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .foregroundStyle(Olive.sap)
                    .background(Olive.cream)
                    .clipShape(Capsule())
            }
        }
    }
}
