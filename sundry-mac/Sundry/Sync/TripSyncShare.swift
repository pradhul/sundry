import AppKit
import Foundation

enum TripSyncShare {
    /// Writes trip bytes to a temp `.sundry` file and opens the AirDrop picker.
    /// The user never sees a save dialog; the temp file is cleaned up later.
    @MainActor
    static func airDrop(data: Data, suggestedName: String) throws {
        let safe = suggestedName
            .replacingOccurrences(of: "/", with: "-")
            .replacingOccurrences(of: ":", with: "-")
        let name = safe.hasSuffix(".sundry") ? safe : "\(safe).sundry"
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("SundryShare", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent(name)
        try data.write(to: url, options: .atomic)

        guard let service = NSSharingService(named: .sendViaAirDrop) else {
            throw ShareError.airDropUnavailable
        }
        guard service.canPerform(withItems: [url]) else {
            throw ShareError.cannotShare
        }
        service.perform(withItems: [url])

        // Keep the file around briefly so AirDrop can read it, then tidy up.
        DispatchQueue.main.asyncAfter(deadline: .now() + 120) {
            try? FileManager.default.removeItem(at: url)
        }
    }

    enum ShareError: LocalizedError {
        case airDropUnavailable
        case cannotShare

        var errorDescription: String? {
            switch self {
            case .airDropUnavailable:
                return "AirDrop isn’t available on this Mac."
            case .cannotShare:
                return "Couldn’t open AirDrop for this trip."
            }
        }
    }
}
