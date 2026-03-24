//! AppState and all related types that cross the FFI boundary.
//! All public structs/enums are annotated with #[derive(uniffi::Record)] or #[derive(uniffi::Enum)].

/// Per-file reading history entry — returned by FfiApp::get_history().
/// NOT embedded in AppState — served via pull method (D-12).
#[derive(uniffi::Record, Clone, Debug)]
pub struct HistoryEntry {
    pub file_hash: String,
    pub file_name: String,
    pub file_path: String,
    pub word_index: u64,
    pub total_words: u64,
    pub progress_percent: f32,  // pre-computed: word_index/total_words*100.0
    pub wpm: u32,
    pub words_per_group: u32,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct AppState {
    pub rev: u64,
    pub router: Router,
    pub display: Option<WordDisplay>,
    pub wpm: u32,
    pub words_per_group: u32,
    pub is_playing: bool,
    pub progress_percent: f32,
    pub current_word_index: u64,
    pub total_words: u64,
    pub is_loading: bool,
    pub error: Option<String>,
    pub toast: Option<String>,
    pub history_revision: u64,  // incremented on any history change; native layers re-call get_history() when this changes
}

impl AppState {
    pub fn initial() -> Self {
        Self {
            rev: 0,
            router: Router {
                default_screen: Screen::Landing,
                screen_stack: vec![],
            },
            display: None,
            wpm: 300,
            words_per_group: 1,
            is_playing: false,
            progress_percent: 0.0,
            current_word_index: 0,
            total_words: 0,
            is_loading: false,
            error: None,
            toast: None,
            history_revision: 0,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct Router {
    pub default_screen: Screen,
    pub screen_stack: Vec<Screen>,
}

impl Router {
    pub fn current_screen(&self) -> &Screen {
        self.screen_stack.last().unwrap_or(&self.default_screen)
    }
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum Screen {
    Landing,
    Reading,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct WordDisplay {
    pub words: Vec<WordSegment>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct WordSegment {
    pub before: String,
    pub anchor: String,
    pub after: String,
}

/// Compute the Optimal Recognition Point anchor for a single word.
/// Splits at floor(word.len() / 2) using char indices (Unicode-safe).
/// Matches the reference web app: Math.floor(word.length / 2) in app.js displayWord.
pub fn compute_orp_anchor(word: &str) -> WordSegment {
    let chars: Vec<char> = word.chars().collect();
    let anchor_idx = chars.len() / 2; // floor division — matches Math.floor
    WordSegment {
        before: chars[..anchor_idx].iter().collect(),
        anchor: chars.get(anchor_idx).map(|c| c.to_string()).unwrap_or_default(),
        after: if anchor_idx + 1 < chars.len() {
            chars[anchor_idx + 1..].iter().collect()
        } else {
            String::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orp_anchor_five_chars() {
        let seg = compute_orp_anchor("hello");
        assert_eq!(seg.before, "he");
        assert_eq!(seg.anchor, "l");
        assert_eq!(seg.after, "lo");
    }

    #[test]
    fn test_orp_anchor_two_chars() {
        let seg = compute_orp_anchor("hi");
        assert_eq!(seg.before, "h");
        assert_eq!(seg.anchor, "i");
        assert_eq!(seg.after, "");
    }

    #[test]
    fn test_orp_anchor_one_char() {
        let seg = compute_orp_anchor("a");
        assert_eq!(seg.before, "");
        assert_eq!(seg.anchor, "a");
        assert_eq!(seg.after, "");
    }

    #[test]
    fn test_orp_anchor_empty() {
        let seg = compute_orp_anchor("");
        assert_eq!(seg.before, "");
        assert_eq!(seg.anchor, "");
        assert_eq!(seg.after, "");
    }

    #[test]
    fn test_orp_anchor_four_chars() {
        // floor(4/2) = 2, so anchor is at index 2 (0-based)
        let seg = compute_orp_anchor("word");
        assert_eq!(seg.before, "wo");
        assert_eq!(seg.anchor, "r");
        assert_eq!(seg.after, "d");
    }
}
