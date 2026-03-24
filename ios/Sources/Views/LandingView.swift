import SwiftUI
import UniformTypeIdentifiers

struct LandingView: View {
    @Bindable var manager: AppManager
    @State private var pasteText: String = ""
    @State private var showFilePicker: Bool = false

    // EPUB has no system UTType constant — construct dynamically (RESEARCH.md Pitfall 4)
    private let epubType = UTType(filenameExtension: "epub") ?? UTType.data

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {  // 16pt landing spacing (UI-SPEC)
            // Title — 36pt semibold, left-aligned (UI-SPEC Typography)
            Text("SpeedReader")
                .font(.system(size: 36, weight: .semibold))

            // Paste area — 180pt height, rounded border (UI-SPEC Screen 1)
            TextEditor(text: $pasteText)
                .frame(height: 180)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
                )
                .overlay(alignment: .topLeading) {
                    if pasteText.isEmpty {
                        Text("Paste text here to start reading...")
                            .foregroundColor(.secondary)
                            .font(.system(size: 16))
                            .padding(.horizontal, 4)
                            .padding(.top, 8)
                            .allowsHitTesting(false)
                    }
                }

            // Button row — 12pt spacing, proportional widths (UI-SPEC Screen 1)
            HStack(spacing: 12) {
                Button("Start Reading") {
                    guard !pasteText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
                    manager.dispatch(.loadText(text: pasteText))
                }
                .buttonStyle(.borderedProminent)
                .frame(maxWidth: .infinity)

                Button("Open File") {
                    showFilePicker = true
                }
                .buttonStyle(.bordered)
                .frame(maxWidth: .infinity)
            }

            // Status area — 14pt, single line (UI-SPEC Screen 1)
            statusView
        }
        .padding(32)  // 32pt screen edge padding (UI-SPEC)
        .frame(maxWidth: 640, alignment: .leading)  // 640pt max width (UI-SPEC)
        .frame(maxWidth: .infinity, alignment: .center)
        .fileImporter(
            isPresented: $showFilePicker,
            allowedContentTypes: [.plainText, .pdf, epubType],
            allowsMultipleSelection: false
        ) { result in
            handleFilePickerResult(result)
        }
    }

    @ViewBuilder
    private var statusView: some View {
        if let error = manager.state.error {
            // Error: {message} at 14pt red (UI-SPEC Copywriting, Color)
            Text("Error: \(error)")
                .font(.system(size: 14))
                .foregroundColor(.red)
        } else if manager.state.isLoading {
            // Loading state — 14pt gray (UI-SPEC Screen 1)
            Text("Loading file...")
                .font(.system(size: 14))
                .foregroundColor(.secondary)
        } else {
            // Empty status line — preserve layout height
            Text(" ")
                .font(.system(size: 14))
        }
    }

    private func handleFilePickerResult(_ result: Result<[URL], Error>) {
        guard case .success(let urls) = result, let url = urls.first else { return }
        Task {
            if let sandboxPath = FileSandboxHelper.copyToSandbox(url: url) {
                // Dispatch sequence: navigate first, then load (Reading screen visible while parsing)
                manager.dispatch(.pushScreen(screen: .reading))
                manager.dispatch(.fileSelected(path: sandboxPath))
            }
            // On copy failure: state.error will be set by Rust or remain nil;
            // no navigation — user sees the status area error if Rust surfaces one.
        }
    }
}
