import SwiftUI

@main
struct SpeedreadingAppApp: App {
    @State private var manager = AppManager()

    var body: some Scene {
        WindowGroup {
            ContentView(manager: manager)
        }
    }
}
