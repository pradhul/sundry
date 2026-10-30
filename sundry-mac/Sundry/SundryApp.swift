import SwiftUI

@main
struct SundryApp: App {
    @State private var store = TripStore()

    var body: some Scene {
        WindowGroup {
            AvailabilityGate {
                ContentView()
                    .environment(store)
            }
            .oliveJournal()
            .onOpenURL { url in
                store.handleIncomingURL(url)
            }
        }
        .handlesExternalEvents(matching: Set(arrayLiteral: "*"))
        .commands {
            CommandGroup(after: .newItem) {
                Button("New Trip") { store.isCreatingTrip = true }
                    .keyboardShortcut("n", modifiers: .command)
            }
        }
    }
}
