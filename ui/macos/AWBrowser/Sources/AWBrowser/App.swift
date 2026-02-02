import SwiftUI

@main
struct AWBrowserApp: App {
    @StateObject private var sessionViewModel = SessionViewModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(sessionViewModel)
        }
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Session...") {
                    sessionViewModel.showLoginSheet = true
                }
                .keyboardShortcut("n", modifiers: .command)
            }

            CommandMenu("Edit") {
                Button("Find and Replace...") {
                    // Show rule editor
                }
                .keyboardShortcut("f", modifiers: [.command, .shift])
            }
        }
    }
}

struct ContentView: View {
    @EnvironmentObject var sessionViewModel: SessionViewModel

    var body: some View {
        if sessionViewModel.isLoggedIn {
            MainView()
        } else {
            LoginView()
        }
    }
}
