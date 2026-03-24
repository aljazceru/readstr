import Foundation

/// Copies a security-scoped URL into the app's sandbox, returning the sandbox path.
///
/// iOS grants time-limited security-scoped access to files outside the app container.
/// The file MUST be copied before stopAccessingSecurityScopedResource() is called.
/// Passing the original URL path to Rust will work in Simulator but fail on physical device.
///
/// - Parameter url: Security-scoped URL from UIDocumentPickerViewController / .fileImporter
/// - Returns: Absolute path string of the sandbox copy, or nil if access or copy failed
enum FileSandboxHelper {
    static func copyToSandbox(url: URL) -> String? {
        // 1. Acquire security-scoped access (no-op in Simulator; required on device)
        let accessing = url.startAccessingSecurityScopedResource()
        defer {
            if accessing { url.stopAccessingSecurityScopedResource() }
        }

        // 2. Resolve destination: applicationSupportDirectory/imports/<filename>
        let fm = FileManager.default
        guard let supportDir = fm.urls(for: .applicationSupportDirectory,
                                        in: .userDomainMask).first else { return nil }
        let importsDir = supportDir.appendingPathComponent("imports", isDirectory: true)

        do {
            try fm.createDirectory(at: importsDir, withIntermediateDirectories: true)
            let destURL = importsDir.appendingPathComponent(url.lastPathComponent)
            // Remove stale copy if re-opening the same filename
            if fm.fileExists(atPath: destURL.path) {
                try fm.removeItem(at: destURL)
            }
            // Copy must happen inside the startAccessing/stopAccessing pair
            try fm.copyItem(at: url, to: destURL)
            return destURL.path
        } catch {
            return nil
        }
    }
}
