import Foundation

struct PlaceHit: Identifiable, Hashable {
    let name: String
    let country: String
    let countryCode: String
    let admin1: String?
    let latitude: Double
    let longitude: Double

    var id: String { "\(latitude),\(longitude),\(countryCode)" }

    var label: String {
        [name, admin1, country]
            .compactMap { $0 }
            .filter { !$0.isEmpty }
            .joined(separator: ", ")
    }
}

/// Open-Meteo requires no API key and no account.
final class OpenMeteoClimateProvider: ClimateTransport, Sendable {
    private struct GeocodeResponse: Decodable {
        struct Result: Decodable {
            let name: String?
            let latitude: Double
            let longitude: Double
            let country: String?
            let country_code: String?
            let admin1: String?
        }
        let results: [Result]?
    }

    private struct ClimateResponse: Decodable {
        struct Daily: Decodable {
            let temperature_2m_max: [Double?]
            let temperature_2m_min: [Double?]
            let precipitation_sum: [Double?]
            let daylight_duration: [Double?]
        }
        let daily: Daily
    }

    func resolve(place: String) async -> [PlaceHit] {
        let trimmed = place.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }

        var components = URLComponents(string: "https://geocoding-api.open-meteo.com/v1/search")!
        components.queryItems = [
            .init(name: "name", value: trimmed),
            .init(name: "count", value: "5"),
            .init(name: "language", value: "en"),
        ]
        guard let url = components.url,
              let (data, _) = try? await URLSession.shared.data(from: url),
              let decoded = try? JSONDecoder().decode(GeocodeResponse.self, from: data)
        else { return [] }

        return (decoded.results ?? []).compactMap { result in
            guard let countryCode = result.country_code, countryCode.count == 2 else { return nil }
            return PlaceHit(
                name: result.name ?? trimmed,
                country: result.country ?? countryCode,
                countryCode: countryCode.uppercased(),
                admin1: result.admin1,
                latitude: result.latitude,
                longitude: result.longitude
            )
        }
    }

    func normals(city: String, countryCode: String, month: UInt32) async -> ClimateNormalsDto? {
        guard let coords = await geocode(city: city, countryCode: countryCode) else { return nil }
        return await normals(at: coords, month: month)
    }

    private func geocode(city: String, countryCode: String) async -> (Double, Double)? {
        var components = URLComponents(string: "https://geocoding-api.open-meteo.com/v1/search")!
        components.queryItems = [
            .init(name: "name", value: city),
            .init(name: "countryCode", value: countryCode),
            .init(name: "count", value: "1"),
        ]
        guard let url = components.url,
              let (data, _) = try? await URLSession.shared.data(from: url),
              let decoded = try? JSONDecoder().decode(GeocodeResponse.self, from: data),
              let first = decoded.results?.first
        else { return nil }
        return (first.latitude, first.longitude)
    }

    /// Uses the most recent complete decade as the climate baseline.
    private func normals(at coords: (Double, Double), month: UInt32) async -> ClimateNormalsDto? {
        let start = String(format: "2014-%02d-01", month)
        let calendar = Calendar(identifier: .gregorian)
        guard let sample = calendar.date(from: DateComponents(year: 2014, month: Int(month), day: 1)),
              let daysInMonth = calendar.range(of: .day, in: .month, for: sample)?.count
        else { return nil }
        let end = String(format: "2023-%02d-%02d", month, daysInMonth)

        var components = URLComponents(string: "https://archive-api.open-meteo.com/v1/archive")!
        components.queryItems = [
            .init(name: "latitude", value: "\(coords.0)"),
            .init(name: "longitude", value: "\(coords.1)"),
            .init(name: "start_date", value: start),
            .init(name: "end_date", value: end),
            .init(name: "daily", value: "temperature_2m_max,temperature_2m_min,precipitation_sum,daylight_duration"),
            .init(name: "timezone", value: "auto"),
        ]

        guard let url = components.url,
              let (data, _) = try? await URLSession.shared.data(from: url),
              let decoded = try? JSONDecoder().decode(ClimateResponse.self, from: data)
        else { return nil }

        func mean(_ values: [Double?]) -> Double {
            let present = values.compactMap { $0 }
            guard !present.isEmpty else { return 0 }
            return present.reduce(0, +) / Double(present.count)
        }

        let daily = decoded.daily
        let monthlyPrecipitation = mean(daily.precipitation_sum) * Double(daysInMonth)

        return ClimateNormalsDto(
            avgHighC: mean(daily.temperature_2m_max),
            avgLowC: mean(daily.temperature_2m_min),
            precipitationMm: monthlyPrecipitation,
            daylightHours: mean(daily.daylight_duration) / 3600.0
        )
    }
}
