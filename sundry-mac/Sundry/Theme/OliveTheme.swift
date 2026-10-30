import SwiftUI

enum Olive {
    /// Warm cream paper — Stitch `mac-main`
    static let paper = Color(red: 244 / 255, green: 239 / 255, blue: 228 / 255)
    /// Deep sap sidebar — Stitch `mac-sidebar`
    static let sap = Color(red: 63 / 255, green: 74 / 255, blue: 46 / 255)
    /// Mid olive accent — Stitch `mac-accent`
    static let accent = Color(red: 92 / 255, green: 107 / 255, blue: 67 / 255)
    /// Warm olive-black ink — Stitch `mac-text-dark`
    static let ink = Color(red: 44 / 255, green: 41 / 255, blue: 37 / 255)
    /// Packed / moss — Stitch `mac-packed`
    static let moss = Color(red: 74 / 255, green: 93 / 255, blue: 35 / 255)
    /// Not-needed / struck — Stitch `mac-strikethrough`
    static let taupe = Color(red: 163 / 255, green: 155 / 255, blue: 140 / 255)
    /// Soft button fill — Stitch `mac-button`
    static let cream = Color(red: 232 / 255, green: 226 / 255, blue: 209 / 255)
    /// Hairline rules — Stitch `mac-border`
    static let rule = Color(red: 216 / 255, green: 211 / 255, blue: 197 / 255)
    /// Button border — Stitch `mac-button-border`
    static let buttonBorder = Color(red: 200 / 255, green: 194 / 255, blue: 177 / 255)

    static func spring(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.35, dampingFraction: 0.8)
    }

    static func softSpring(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.48, dampingFraction: 0.86)
    }

    static func snappy(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.28, dampingFraction: 0.72)
    }

    static var packTransition: AnyTransition {
        .asymmetric(
            insertion: .move(edge: .trailing).combined(with: .opacity).combined(with: .scale(scale: 0.96)),
            removal: .move(edge: .leading).combined(with: .opacity).combined(with: .scale(scale: 0.96))
        )
    }
}

struct OlivePrimaryButtonStyle: ButtonStyle {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.body.weight(.semibold))
            .tracking(0.3)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .foregroundStyle(Olive.paper)
            .background(Olive.sap.opacity(configuration.isPressed ? 0.85 : 1))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .scaleEffect(configuration.isPressed && !reduceMotion ? 0.97 : 1)
            .animation(Olive.snappy(reduceMotion), value: configuration.isPressed)
    }
}

struct OliveToolbarButtonStyle: ButtonStyle {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var emphasized = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.callout.weight(.medium))
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .foregroundStyle(emphasized ? Olive.accent : Olive.ink)
            .background(Olive.cream.opacity(configuration.isPressed ? 0.7 : 1))
            .overlay(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .strokeBorder(Olive.buttonBorder, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
            .shadow(color: .black.opacity(0.04), radius: 1, y: 1)
            .scaleEffect(configuration.isPressed && !reduceMotion ? 0.97 : 1)
            .animation(Olive.snappy(reduceMotion), value: configuration.isPressed)
    }
}

struct OliveJournalModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .tint(Olive.accent)
            .preferredColorScheme(.light)
    }
}

extension View {
    func oliveJournal() -> some View {
        modifier(OliveJournalModifier())
    }
}
