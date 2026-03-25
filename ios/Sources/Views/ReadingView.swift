import SwiftUI

struct ReadingView: View {
    @Bindable var manager: AppManager

    // Local preview values — prevent saturating Rust actor during slider drag
    // (RESEARCH.md Pattern 7, UI-SPEC Interaction Contracts)
    @State private var wpmPreview: Double = 300
    @State private var groupPreview: Double = 1
    @State private var seekPreview: Double = 0

    @AppStorage("dark_mode") private var darkMode: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {  // 20pt section spacing (UI-SPEC)
            // RSVP word display area — systemFill rounded background, 24pt/32pt internal padding (UI-SPEC D-04, D-18)
            wordDisplayArea
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
                .padding(.horizontal, 32)
                .background(
                    Color(.systemFill),
                    in: RoundedRectangle(cornerRadius: 12)
                )
                .frame(minHeight: 120)

            // Controls row: Play/Pause + Seek bar — 12pt spacing (UI-SPEC)
            HStack(spacing: 12) {
                Button(action: { manager.dispatch(.toggle) }) {
                    Label(
                        manager.state.isPlaying ? "Pause" : "Play",
                        systemImage: manager.state.isPlaying ? "pause.fill" : "play.fill"
                    )
                }
                .buttonStyle(.borderedProminent)

                // Seek bar — dispatches on drag commit only (UI-SPEC Interaction Contracts)
                Slider(
                    value: Binding(
                        get: { Double(manager.state.progressPercent) },
                        set: { newValue in seekPreview = newValue }
                    ),
                    in: 0...100
                ) { editing in
                    if !editing {
                        manager.dispatch(.seekToProgress(percent: Float(seekPreview)))
                    }
                }
                .accessibilityLabel("Reading progress")
                .accessibilityValue("\(Int(manager.state.progressPercent)) percent")
            }

            // Slider row: WPM + Group — 12pt spacing (UI-SPEC)
            HStack(spacing: 12) {
                // WPM label — 16pt, fixed width ~80pt (UI-SPEC)
                Text("\(Int(wpmPreview)) WPM")
                    .font(.system(size: 16))
                    .frame(width: 80, alignment: .leading)

                // WPM slider — 100...1000, step 10, release-only dispatch (UI-SPEC)
                Slider(value: $wpmPreview, in: 100...1000, step: 10) { editing in
                    if !editing {
                        manager.dispatch(.setWpm(wpm: UInt32(wpmPreview)))
                    }
                }
                .accessibilityLabel("Reading speed")
                .accessibilityValue("\(Int(wpmPreview)) words per minute")

                // Group label — 16pt, fixed width ~32pt (UI-SPEC)
                Text("×\(Int(groupPreview))")
                    .font(.system(size: 16))
                    .frame(width: 32, alignment: .leading)

                // Group slider — 1...5, step 1, release-only dispatch (UI-SPEC)
                Slider(value: $groupPreview, in: 1...5, step: 1) { editing in
                    if !editing {
                        manager.dispatch(.setWordsPerGroup(n: UInt32(groupPreview)))
                    }
                }
                .accessibilityLabel("Words per group")
                .accessibilityValue("\(Int(groupPreview)) words")
            }

            // Replay row — visible only at end-of-document (UI-SPEC replay condition)
            if manager.state.progressPercent >= 99.9
                && !manager.state.isPlaying
                && manager.state.totalWords > 0 {
                Button(action: { manager.dispatch(.replay) }) {
                    Label("Replay", systemImage: "arrow.counterclockwise")
                }
                .buttonStyle(.bordered)
            }
        }
        .padding(32)  // 32pt screen edge padding (UI-SPEC)
        .frame(maxWidth: 720)  // 720pt max width (UI-SPEC)
        .frame(maxWidth: .infinity, alignment: .center)
        .navigationTitle("")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            // Back button — leading (UI-SPEC; custom back overrides NavigationStack default)
            ToolbarItem(placement: .navigationBarLeading) {
                Button(action: { manager.dispatch(.popScreen) }) {
                    Label("Back", systemImage: "chevron.left")
                }
            }
            // Theme toggle — trailing (UI-SPEC Screen 2)
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(darkMode ? "Light Mode" : "Dark Mode") {
                    darkMode.toggle()
                }
            }
        }
        .navigationBarBackButtonHidden(true)  // Using custom Back button above
        .onAppear {
            // Sync preview values from Rust state on appear
            syncPreviewValues()
        }
        .onChange(of: manager.state.rev) { _, _ in
            // Re-sync preview values when Rust state updates (rev-guarded in AppManager)
            syncPreviewValues()
        }
    }

    // MARK: - RSVP Word Display

    @ViewBuilder
    private var wordDisplayArea: some View {
        if manager.state.isLoading {
            Text("Loading...")
                .font(.system(size: 48, weight: .medium))
                .frame(maxWidth: .infinity)
                .multilineTextAlignment(.center)
        } else if let display = manager.state.display, !display.words.isEmpty {
            Text(buildAttributedString(from: display))
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity)
                .accessibilityLabel(fullWordString(from: display))
        } else {
            // Empty/idle state — em dash (UI-SPEC Copywriting)
            Text("—")
                .font(.system(size: 48, weight: .medium))
                .frame(maxWidth: .infinity)
                .multilineTextAlignment(.center)
        }
    }

    /// Build AttributedString with ORP anchor highlighted in orange.
    /// Before/after: 48pt medium. Anchor: 48pt bold orange. (UI-SPEC Typography + Color)
    private func buildAttributedString(from display: WordDisplay) -> AttributedString {
        var attributed = AttributedString()

        for (i, seg) in display.words.enumerated() {
            // Before segment — medium weight
            var before = AttributedString(seg.before)
            before.font = .system(size: 48, weight: .medium)
            attributed.append(before)

            // Anchor letter — bold, orange (accent color reserved for ORP only — UI-SPEC Color)
            var anchor = AttributedString(seg.anchor)
            anchor.font = .system(size: 48, weight: .bold)
            anchor.foregroundColor = .orange
            attributed.append(anchor)

            // After segment — medium weight
            var after = AttributedString(seg.after)
            after.font = .system(size: 48, weight: .medium)
            attributed.append(after)

            // Space between words in multi-word group
            if i < display.words.count - 1 {
                var space = AttributedString(" ")
                space.font = .system(size: 48)
                attributed.append(space)
            }
        }
        return attributed
    }

    /// Accessibility: full word string for VoiceOver (UI-SPEC Accessibility)
    private func fullWordString(from display: WordDisplay) -> String {
        display.words.map { seg in seg.before + seg.anchor + seg.after }.joined(separator: " ")
    }

    // MARK: - Preview Sync

    private func syncPreviewValues() {
        wpmPreview = Double(manager.state.wpm)
        groupPreview = Double(manager.state.wordsPerGroup)
        // seekPreview synced from state via Binding.get — no explicit sync needed here
        seekPreview = Double(manager.state.progressPercent)
    }
}
