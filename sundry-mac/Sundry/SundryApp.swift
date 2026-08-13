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
        }
        .commands {
            CommandGroup(after: .newItem) {
                Button("New Trip") { store.isCreatingTrip = true }
                    .keyboardShortcut("n", modifiers: .command)
            }
        }
    }
}
