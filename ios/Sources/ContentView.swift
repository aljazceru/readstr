import SwiftUI

struct ContentView: View {
    @Bindable var manager: AppManager

    // Local nav path mirrors manager.state.router.screenStack from Rust
    @State private var navPath: [Screen] = []

    var body: some View {
        NavigationStack(path: $navPath) {
            LandingView(manager: manager)
                .navigationDestination(for: Screen.self) { screen in
                    switch screen {
                    case .reading:
                        ReadingView(manager: manager)
                    case .landing:
                        // Landing is the root — popping to it is handled by navPath sync
                        LandingView(manager: manager)
                    @unknown default:
                        EmptyView()
                    }
                }
        }
        // Rust router is authoritative: sync navPath when Rust state changes
        .onChange(of: manager.state.router.screenStack) { _, newStack in
            navPath = newStack
        }
        // User back-swipe: navPath shrinks before Rust knows — dispatch PopScreen
        .onChange(of: navPath) { old, new in
            guard new != manager.state.router.screenStack else { return }
            if new.count < old.count {
                manager.dispatch(.popScreen)
            }
        }
    }
}
