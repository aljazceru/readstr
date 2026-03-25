# Speedreading App

A cross-platform RSVP (Rapid Serial Visual Presentation) speed reading app for iOS, Android, and Desktop. Words flash at your chosen WPM with an anchor-letter focus point to guide the eye. Load a plain text file, EPUB, PDF, or paste text directly.

Built with the [RMP (Rust Multiplatform)](https://github.com/nickthecook/rmp) architecture — shared Rust core, native UI layers.

## Platform Support

| Platform | Minimum Version | UI Framework |
|----------|----------------|--------------|
| iOS      | 16+            | Swift / SwiftUI |
| Android  | API 26+ (Android 8) | Kotlin / Jetpack Compose |
| Desktop  | Linux / macOS / Windows | iced 0.14 |

## Architecture

A shared Rust core (`speedreading-app_core`) implements all domain logic: file parsing, tokenisation, RSVP playback engine, and state management. Platform layers (Swift, Kotlin, iced) contain no business logic — they render state and forward events.

- **FFI bindings**: UniFFI 0.31 generates Swift and Kotlin bindings from annotated Rust types
- **Data flow**: Unidirectional (Elm/TEA architecture) — no shared mutable state across the FFI boundary
- **File parsing**: pure-Rust crates (`epub`, `pdf-extract`) cross-compile to all targets
- **Persistence**: SQLite via `rusqlite` (bundled feature) — reading position, settings

## Prerequisites

**All platforms**

- [Nix](https://nixos.org/download) with flakes enabled — provides the full toolchain via `flake.nix`
- Or manually: Rust stable toolchain, `just`, `rmp` CLI

**iOS** (macOS only)

- Xcode 15+
- `xcodegen` (included in the Nix shell)

**Android**

- Android SDK with NDK r25+
- Set `ANDROID_HOME` or write `sdk.dir=<path>` in `android/local.properties` (gitignored)

## Build Instructions

Enter the Nix dev shell first:

```bash
nix develop
# or with direnv: direnv allow  (then the shell activates automatically)
```

### Verify prerequisites

```bash
just doctor
```

### Generate FFI bindings (required before first platform build)

```bash
just bindings
```

### Desktop

```bash
just run-iced       # build and run the desktop app
```

### Android

```bash
just android-full   # full pipeline: bindings + cross-compile Rust + assemble APK
just run-android    # deploy and run on connected device or emulator
```

### iOS (macOS only)

```bash
just ios-full       # full pipeline: bindings + cross-compile + xcframework + xcodegen + build
just run-ios        # run on simulator
```

Run `just` with no arguments to list all available recipes.

## Project Status

Early stage — core RSVP playback and file loading are the current focus. UI is functional but minimal.
