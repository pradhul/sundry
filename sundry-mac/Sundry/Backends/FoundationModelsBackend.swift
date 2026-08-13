import FoundationModels
import Foundation

enum ModelAvailability {
    case available
    case notEnabled
    case downloading
    case ineligible(String)
}

func currentAvailability() -> ModelAvailability {
    switch SystemLanguageModel.default.availability {
    case .available:
        return .available
    case .unavailable(.appleIntelligenceNotEnabled):
        return .notEnabled
    case .unavailable(.modelNotReady):
        return .downloading
    case .unavailable(let other):
        return .ineligible("\(other)")
    @unknown default:
        return .ineligible("unknown availability")
    }
}

final class FoundationModelsBackend: LlmTransport, Sendable {
    func generate(prompt: String) async throws -> String {
        guard case .available = currentAvailability() else {
            throw SundryError.Core(message: "the on-device model is unavailable")
        }
        let session = LanguageModelSession()
        let response = try await session.respond(to: prompt, generating: GeneratedListSchema.self)
        let data = try JSONEncoder().encode(response.content)
        guard let json = String(data: data, encoding: .utf8) else {
            throw SundryError.Core(message: "could not encode model response")
        }
        return json
    }
}
