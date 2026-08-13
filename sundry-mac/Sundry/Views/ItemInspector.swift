import SwiftUI

struct ItemInspector: View {
    let item: ItemDto?

    var body: some View {
        Group {
            if let item {
                VStack(alignment: .leading, spacing: 14) {
                    Text(item.name).font(.title3.weight(.semibold))
                    LabeledContent("Category", value: item.category)
                    LabeledContent("Quantity", value: "\(item.quantity)")
                    if let rationale = item.rationale {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Why").font(.caption.weight(.semibold))
                                .foregroundStyle(.secondary)
                            Text(rationale)
                        }
                    }
                    Spacer()
                }
                .padding()
            } else {
                ContentUnavailableView("No item selected", systemImage: "sidebar.right")
            }
        }
        .frame(minWidth: 240)
    }
}
