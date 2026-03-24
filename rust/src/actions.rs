//! All user intents that can be dispatched to the Rust core.
//! AppAction crosses the FFI boundary — all variants annotated with uniffi::Enum.

use crate::state::Screen;

#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppAction {
    // Navigation
    PushScreen { screen: Screen },
    PopScreen,
    // Input
    LoadText { text: String },
    FileSelected { path: String },
    // Playback
    Play,
    Pause,
    Toggle,
    SeekToProgress { percent: f32 },
    SetWPM { wpm: u32 },
    SetWordsPerGroup { n: u32 },
    Replay,
    // UI
    ClearToast,
    ClearError,
    // Lifecycle
    Foregrounded,
}
