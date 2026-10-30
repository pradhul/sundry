import SwiftUI

struct AddItemSheet: View {
    @Environment(TripStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var name = ""
    @State private var category = ""
    @State private var quantity = 1.0
    @State private var essential = false
    @State private var errorMessage: String?

    private var trimmed: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var isValid: Bool {
        !trimmed.isEmpty && !category.isEmpty
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Add item")
                    .font(.title3.weight(.semibold))
                    .tracking(0.4)
                Spacer()
            }
            .foregroundStyle(Olive.paper)
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            .background(Olive.sap)

            Form {
                TextField("Name", text: $name, prompt: Text("SPF 50 sunscreen"))
                Picker("Category", selection: $category) {
                    ForEach(store.orderedCategories, id: \.self) { cat in
                        Text(cat).tag(cat)
                    }
                }
                Stepper("Quantity: \(Int(quantity))", value: $quantity, in: 1...60)
                Toggle("Essential", isOn: $essential)
                    .tint(Olive.accent)

                if let errorMessage {
                    Text(errorMessage)
                        .font(.callout)
                        .foregroundStyle(.red)
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)
            .onAppear {
                if category.isEmpty {
                    category = store.orderedCategories.first ?? "Miscellaneous"
                }
            }

            HStack {
                Button("Cancel") { store.isAddingItem = false }
                    .buttonStyle(.plain)
                    .foregroundStyle(Olive.sap)
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Add") { submit() }
                    .buttonStyle(OlivePrimaryButtonStyle())
                    .keyboardShortcut(.defaultAction)
                    .disabled(!isValid)
                    .scaleEffect(isValid && !reduceMotion ? 1 : 0.98)
                    .animation(Olive.spring(reduceMotion), value: isValid)
            }
            .padding(20)
        }
        .background(Olive.paper)
        .frame(width: 420)
        .oliveJournal()
    }

    private func submit() {
        do {
            try store.addManualItem(
                name: trimmed,
                category: category,
                quantity: UInt32(quantity),
                essential: essential
            )
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
