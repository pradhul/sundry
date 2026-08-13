import SwiftUI

struct NewTripSheet: View {
    @Environment(TripStore.self) private var store

    @State private var city = ""
    @State private var countryCode = ""
    @State private var month = UInt32(Calendar.current.component(.month, from: .now))
    @State private var year = Int32(Calendar.current.component(.year, from: .now))
    @State private var nights = 7.0
    @State private var travellers = 2.0
    @State private var purposes: Set<String> = ["leisure"]

    private let allPurposes = ["business", "leisure", "hiking", "beach", "city"]

    private var isValid: Bool {
        !city.trimmingCharacters(in: .whitespaces).isEmpty && countryCode.count == 2
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("New Trip").font(.title2.weight(.semibold))

            Form {
                TextField("City", text: $city, prompt: Text("Kyoto"))
                TextField("Country code", text: $countryCode, prompt: Text("JP"))
                    .onChange(of: countryCode) { _, new in
                        countryCode = String(new.uppercased().prefix(2))
                    }
                Picker("Month", selection: $month) {
                    ForEach(1...12, id: \.self) { m in
                        Text(monthName(UInt32(m))).tag(UInt32(m))
                    }
                }
                Stepper("Year: \(String(year))", value: $year, in: 2026...2035)
                Stepper("Nights: \(Int(nights))", value: $nights, in: 1...90)
                Stepper("Travellers: \(Int(travellers))", value: $travellers, in: 1...8)

                LabeledContent("Purpose") {
                    HStack {
                        ForEach(allPurposes, id: \.self) { p in
                            Toggle(p.capitalized, isOn: Binding(
                                get: { purposes.contains(p) },
                                set: { on in
                                    if on { purposes.insert(p) } else { purposes.remove(p) }
                                }
                            ))
                            .toggleStyle(.button)
                        }
                    }
                }
            }
            .formStyle(.grouped)

            HStack {
                Spacer()
                Button("Cancel") { store.isCreatingTrip = false }
                    .keyboardShortcut(.cancelAction)
                Button("Create and Generate") {
                    store.createTrip(
                        title: city,
                        city: city,
                        countryCode: countryCode,
                        year: year,
                        month: month,
                        nights: UInt32(nights),
                        travellers: (1...Int(travellers)).map { "Traveller \($0)" },
                        purposes: Array(purposes)
                    )
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!isValid)
            }
        }
        .padding(20)
        .frame(width: 460)
    }
}
