import SwiftUI

@main
struct SpeedreadingAppApp: App {
    @State private var manager = AppManager()
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("dark_mode") private var darkMode: Bool = false

    var body: some Scene {
        WindowGroup {
            ContentView(manager: manager)
                .preferredColorScheme(darkMode ? .dark : .light)
        }
        .onChange(of: scenePhase) { _, newPhase in
            switch newPhase {
            case .background:
                // App fully left foreground — pause any active playback
                manager.dispatch(.pause)
            case .active:
                // App returned to foreground — signal Rust to resume if applicable
                manager.dispatch(.foregrounded)
            case .inactive:
                // Transient: Control Center, notification shade, multitasking gesture.
                // Do NOT dispatch Pause here — causes spurious pauses during normal use.
                break
            @unknown default:
                break
            }
        }
    }
}
