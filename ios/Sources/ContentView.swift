import SwiftUI

struct ContentView: View {
    @Environment(AppManager.self) var manager

    var body: some View {
        Text("SpeedReader")
            .font(.largeTitle)
    }
}
