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
    /// The on-device context is 8192 tokens for prompt + response. Prompts are
    /// ~1-2k tokens; an 8-item list needs well under 1.5k.
    static let maxResponseTokens = 1500

    func generate(prompt: String) async throws -> String {
        guard case .available = currentAvailability() else {
            throw SundryError.Core(message: "the on-device model is unavailable")
        }
        do {
            return try await respond(to: prompt)
        } catch LanguageModelSession.GenerationError.exceededContextWindowSize {
            // Fresh session, shorter prompt, one more try.
            do {
                return try await respond(to: Self.trim(prompt))
            } catch {
                throw SundryError.Core(message: "model failed: \(error)")
            }
        } catch let error as SundryError {
            throw error
        } catch {
            throw SundryError.Core(message: "model failed: \(error)")
        }
    }

    /// Always a fresh session: nothing accumulates across categories or retries.
    private func respond(to prompt: String) async throws -> String {
        let session = LanguageModelSession()
        let options = GenerationOptions(maximumResponseTokens: Self.maxResponseTokens)
        let response = try await session.respond(
            to: prompt, generating: GeneratedListSchema.self, options: options)
        let data = try JSONEncoder().encode(response.content)
        guard let json = String(data: data, encoding: .utf8) else {
            throw SundryError.Core(message: "could not encode model response")
        }
        return json
    }

    /// Drops the "already on the list" block, the only part that grows.
    static func trim(_ prompt: String) -> String {
        var lines = prompt.components(separatedBy: "\n")
        if let start = lines.firstIndex(where: { $0.hasPrefix("Already on the packing list") }) {
            var end = start + 1
            while end < lines.count, lines[end].hasPrefix("- ") { end += 1 }
            lines.removeSubrange(start..<end)
        }
        return lines.joined(separator: "\n")
    }
}
