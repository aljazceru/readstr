import SwiftUI
import UniformTypeIdentifiers

struct LandingView: View {
    @Bindable var manager: AppManager
    @State private var pasteText: String = ""
    @State private var showFilePicker: Bool = false
    @State private var showDeleteConfirm: Bool = false
    @State private var pendingDeleteEntry: HistoryEntryUi? = nil
    @State private var showRelocatePicker: Bool = false
    @State private var fileNotFoundError: String? = nil

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

            // History section — hidden when empty (D-01, D-03)
            historySection
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
        .fileImporter(
            isPresented: $showRelocatePicker,
            allowedContentTypes: [.plainText, .pdf, epubType],
            allowsMultipleSelection: false
        ) { result in
            if case .success = result {
                fileNotFoundError = nil
            }
            handleFilePickerResult(result)
        }
        .alert(
            pendingDeleteEntry.map { "Delete entry for \($0.entry.fileName)?" } ?? "Delete entry?",
            isPresented: $showDeleteConfirm
        ) {
            Button("Delete", role: .destructive) {
                if let entry = pendingDeleteEntry {
                    manager.dispatch(.deleteSession(fileHash: entry.entry.fileHash))
                }
                pendingDeleteEntry = nil
            }
            Button("Keep Entry", role: .cancel) {
                pendingDeleteEntry = nil
            }
        }
    }

    @ViewBuilder
    private var statusView: some View {
        if let error = manager.state.error {
            // Error: {message} at 14pt red (UI-SPEC Copywriting, Color)
            Text("Error: \(error)")
                .font(.system(size: 14))
                .foregroundColor(.red)
        } else if let error = fileNotFoundError {
            Text(error)
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

    @ViewBuilder
    private var historySection: some View {
        if !manager.history.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Recent Files")
                    .font(.system(size: 14))
                    .foregroundColor(.secondary)

                // Scrollable list of history entries
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(manager.history, id: \.entry.fileHash) { item in
                            historyRow(item)
                            Divider()
                        }
                    }
                }
            }
        }
    }

    private func historyRow(_ item: HistoryEntryUi) -> some View {
        HStack(spacing: 8) {
            // Icon — warning for missing, document for normal (D-08, D-11)
            Text(item.isMissing ? "⚠️" : "📄")
                .accessibilityLabel(item.isMissing ? "File not found" : "Document")

            // File name + optional "File not found" sublabel
            VStack(alignment: .leading, spacing: 2) {
                Text(item.entry.fileName)
                    .font(.body)
                    .foregroundColor(item.isMissing ? .secondary : .primary)
                if item.isMissing {
                    Text("File not found")
                        .font(.system(size: 14))
                        .foregroundColor(.secondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            // Progress % — integer, no decimal (UI-SPEC Pitfall 6)
            Text("\(Int(item.entry.progressPercent))%")
                .font(.system(size: 14))
                .accessibilityLabel("\(Int(item.entry.progressPercent)) percent")

            // Resume button (D-12)
            Button("Resume") {
                if item.isMissing {
                    // D-10: show error + open picker immediately; do NOT dispatch ResumeFile
                    fileNotFoundError = "File not found — please re-locate it"
                    showRelocatePicker = true
                } else {
                    fileNotFoundError = nil
                    // Do NOT dispatch .pushScreen(.reading) — on_parse_complete pushes it (Pitfall 1)
                    manager.dispatch(.resumeFile(fileHash: item.entry.fileHash))
                }
            }
            .buttonStyle(.borderedProminent)
            .frame(minHeight: 44)  // accessibility touch target

            // Trash icon (D-06) — shows confirmation alert (D-07)
            Button {
                pendingDeleteEntry = item
                showDeleteConfirm = true
            } label: {
                Image(systemName: "trash")
            }
            .accessibilityLabel("Delete \(item.entry.fileName) history entry")
            .foregroundColor(.secondary)
            .frame(minWidth: 44, minHeight: 44)  // accessibility touch target
        }
        .padding(.vertical, 8)
        .frame(minHeight: 44)
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
