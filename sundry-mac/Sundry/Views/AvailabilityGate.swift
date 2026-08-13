import SwiftUI
import AppKit

struct AvailabilityGate<Content: View>: View {
    @ViewBuilder var content: () -> Content
    @State private var availability = currentAvailability()

    var body: some View {
        switch availability {
        case .available:
            content()
        case .notEnabled:
            message(
                title: "Turn on Apple Intelligence",
                detail: "Sundry builds your packing list on this Mac using Apple's on-device model. Enable Apple Intelligence in System Settings to continue.",
                action: "Open System Settings"
            ) {
                if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security") {
                    NSWorkspace.shared.open(url)
                }
            }
        case .downloading:
            message(
                title: "Preparing the model",
                detail: "macOS is still downloading the on-device model. This finishes in the background — check back shortly.",
                action: "Check again"
            ) {
                availability = currentAvailability()
            }
        case .ineligible(let reason):
            message(
                title: "Not supported on this Mac",
                detail: "Sundry needs an Apple Silicon Mac with Apple Intelligence. (\(reason))",
                action: nil,
                perform: nil
            )
        }
    }

    @ViewBuilder
    private func message(
        title: String,
        detail: String,
        action: String?,
        perform: (() -> Void)?
    ) -> some View {
        VStack(spacing: 12) {
            Image(systemName: "sparkles").font(.largeTitle)
            Text(title).font(.title2.weight(.semibold))
            Text(detail)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .frame(maxWidth: 380)
            if let action, let perform {
                Button(action, action: perform).buttonStyle(.borderedProminent)
            }
        }
        .padding(40)
        .frame(minWidth: 480, minHeight: 320)
    }
}
