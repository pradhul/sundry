import Foundation

/// Watches the user's Downloads folder for newly arrived `.sundry` files and
/// imports them into the app automatically — so AirDrop → Downloads → Sync
/// without a manual file picker.
@MainActor
final class TripInboxWatcher {
    private var source: DispatchSourceFileSystemObject?
    private var directoryFD: CInt = -1
    private var seen = Set<String>()
    private var onFile: ((URL) -> Void)?

    func start(onFile: @escaping (URL) -> Void) {
        stop()
        self.onFile = onFile

        let downloads = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        guard let downloads else { return }

        // Seed known files so we only react to new arrivals.
        if let existing = try? FileManager.default.contentsOfDirectory(
            at: downloads,
            includingPropertiesForKeys: [.contentModificationDateKey],
            options: [.skipsHiddenFiles]
        ) {
            for url in existing where url.pathExtension.lowercased() == "sundry" {
                seen.insert(fingerprint(url))
            }
        }

        let path = (downloads as NSURL).fileSystemRepresentation
        directoryFD = open(path, O_EVTONLY)
        guard directoryFD >= 0 else { return }

        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: directoryFD,
            eventMask: [.write, .rename, .extend],
            queue: DispatchQueue.main
        )
        source.setEventHandler { [weak self] in
            self?.scan(downloads)
        }
        source.setCancelHandler { [weak self] in
            if let self, self.directoryFD >= 0 {
                close(self.directoryFD)
                self.directoryFD = -1
            }
        }
        self.source = source
        source.resume()

        // Catch files that landed while we were starting.
        scan(downloads)
    }

    func stop() {
        source?.cancel()
        source = nil
        onFile = nil
        seen.removeAll()
    }

    private func scan(_ downloads: URL) {
        guard let files = try? FileManager.default.contentsOfDirectory(
            at: downloads,
            includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else { return }

        let now = Date()
        for url in files where url.pathExtension.lowercased() == "sundry" {
            let key = fingerprint(url)
            guard !seen.contains(key) else { continue }

            // Only auto-import recent files (AirDrop arrivals), not old clutter.
            let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .isRegularFileKey])
            guard values?.isRegularFile == true else { continue }
            if let modified = values?.contentModificationDate,
               now.timeIntervalSince(modified) > 60 * 30 {
                seen.insert(key)
                continue
            }

            seen.insert(key)
            onFile?(url)
        }
    }

    private func fingerprint(_ url: URL) -> String {
        let values = try? url.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey])
        let size = values?.fileSize ?? 0
        let date = values?.contentModificationDate?.timeIntervalSince1970 ?? 0
        return "\(url.lastPathComponent)|\(size)|\(date)"
    }
}
