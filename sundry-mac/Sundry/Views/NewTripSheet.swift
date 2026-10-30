import SwiftUI

struct NewTripSheet: View {
    @Environment(TripStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var place = ""
    @State private var countryName = ""
    @State private var month = UInt32(Calendar.current.component(.month, from: .now))
    @State private var year = Int32(Calendar.current.component(.year, from: .now))
    @State private var nights = 7.0
    @State private var purposes: Set<String> = ["leisure"]

    @State private var hits: [PlaceHit] = []
    @State private var selectedHit: PlaceHit?
    @State private var showCountryFallback = false
    @State private var isResolving = false
    @State private var errorMessage: String?

    private let geocoder = OpenMeteoClimateProvider()
    private let allPurposes = ["business", "leisure", "hiking", "beach", "city"]

    private var trimmedPlace: String {
        place.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var isValid: Bool {
        guard !trimmedPlace.isEmpty, !purposes.isEmpty else { return false }
        if showCountryFallback {
            return !countryName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        if hits.count > 1 {
            return selectedHit != nil
        }
        return true
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("New trip")
                    .font(.title3.weight(.semibold))
                    .tracking(0.4)
                Spacer()
            }
            .foregroundStyle(Olive.paper)
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            .background(Olive.sap)

            Form {
                TextField("Place", text: $place, prompt: Text("Bali, Kyoto, Lisbon…"))
                    .onChange(of: place) { _, _ in
                        hits = []
                        selectedHit = nil
                        errorMessage = nil
                    }

                if hits.count > 1 {
                    Picker("Which one?", selection: $selectedHit) {
                        ForEach(hits) { hit in
                            Text(hit.label).tag(Optional(hit))
                        }
                    }
                }

                if showCountryFallback {
                    TextField("Country", text: $countryName, prompt: Text("Indonesia"))
                    Text("We couldn’t find that place. Add the country name and try again.")
                        .font(.caption)
                        .foregroundStyle(Olive.taupe)
                }

                if let errorMessage {
                    Text(errorMessage)
                        .font(.callout)
                        .foregroundStyle(.red)
                }

                Picker("Month", selection: $month) {
                    ForEach(1...12, id: \.self) { m in
                        Text(monthName(UInt32(m))).tag(UInt32(m))
                    }
                }
                Stepper("Year: \(String(year))", value: $year, in: 2026...2035)
                Stepper("Nights: \(Int(nights))", value: $nights, in: 1...90)

                Text("Packing for \(store.travellerNames.joined(separator: " & "))")
                    .font(.callout)
                    .foregroundStyle(Olive.taupe)

                LabeledContent("Purpose") {
                    HStack {
                        ForEach(allPurposes, id: \.self) { p in
                            let on = purposes.contains(p)
                            Button(p.capitalized) {
                                if on { purposes.remove(p) } else { purposes.insert(p) }
                            }
                            .buttonStyle(.plain)
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .foregroundStyle(on ? Olive.paper : Olive.sap)
                            .background(on ? Olive.sap : Olive.cream)
                            .clipShape(Capsule())
                            .scaleEffect(on && !reduceMotion ? 1.04 : 1)
                            .animation(Olive.spring(reduceMotion), value: on)
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)

            HStack {
                Button("Cancel") { store.isCreatingTrip = false }
                    .buttonStyle(.plain)
                    .foregroundStyle(Olive.sap)
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button(isResolving ? "Finding place…" : "Create and Generate") {
                    Task { await submit() }
                }
                .buttonStyle(OlivePrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
                .disabled(!isValid || isResolving)
            }
            .padding(20)
        }
        .background(Olive.paper)
        .frame(width: 500)
        .oliveJournal()
    }

    private func submit() async {
        errorMessage = nil
        if let selectedHit {
            create(from: selectedHit)
            return
        }

        isResolving = true
        defer { isResolving = false }

        var query = trimmedPlace
        let country = countryName.trimmingCharacters(in: .whitespacesAndNewlines)
        if showCountryFallback, !country.isEmpty {
            query = "\(trimmedPlace) \(country)"
        }

        let found = await geocoder.resolve(place: query)
        if found.isEmpty {
            if showCountryFallback {
                errorMessage = "Still couldn’t find that place. Try a city name, like “Denpasar” or “Ubud”."
            } else {
                showCountryFallback = true
            }
            return
        }
        if found.count == 1, let only = found.first {
            create(from: only)
            return
        }
        hits = found
        selectedHit = found.first
    }

    private func create(from hit: PlaceHit) {
        do {
            try store.createTrip(
                title: hit.name,
                city: hit.name,
                countryCode: hit.countryCode,
                year: year,
                month: month,
                nights: UInt32(nights),
                purposes: Array(purposes)
            )
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
