//! Actor thread logic: ActorState, action handlers, playback engine.
//! The actor thread is a plain std::thread running a blocking loop.
//! It owns a tokio Runtime for async I/O (file parsing, playback timer).

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use flume::Sender;

use crate::{
    actions::AppAction,
    core::{
        parser::detect_and_parse,
        session::{open_db, restore_session, save_session, SessionData},
    },
    state::{AppState, Screen, WordDisplay, compute_orp_anchor},
    updates::{AppUpdate, CoreMsg, InternalEvent},
};

/// Actor-local state — never serialized, never crosses FFI.
pub struct ActorState {
    /// The current word array — the critical performance invariant:
    /// lives here only, NEVER in AppState or Arc<RwLock<>>.
    pub words: Vec<String>,
    /// Playback cancel handle — drop to stop timer task.
    pub playback_cancel: Option<tokio::sync::oneshot::Sender<()>>,
    /// Playback start time for Instant-based drift correction.
    pub playback_start: Option<Instant>,
    /// Word index at which playback last started (for resume-from-position).
    pub playback_start_index: u64,
    /// rusqlite connection — None until data_dir is known (set in FfiApp::new).
    pub db: Option<rusqlite::Connection>,
    /// The AppState that will be emitted. Actor mutates this directly.
    pub state: AppState,
    /// True if playback was active when the app entered the background.
    /// Set by BackgroundPause (lifecycle-triggered); cleared on Foregrounded.
    pub was_playing_before_background: bool,
}

impl ActorState {
    pub fn new(data_dir: &str) -> Self {
        let db = open_db(data_dir).ok();
        let mut state = AppState::initial();

        // Restore WPM and words_per_group from session (if exists)
        // Word position restored after text is loaded (need text hash to validate)
        if let Some(ref conn) = db {
            if let Ok(Some(session)) = restore_session(conn) {
                state.wpm = session.wpm;
                state.words_per_group = session.words_per_group;
            }
        }

        Self {
            words: vec![],
            playback_cancel: None,
            playback_start: None,
            playback_start_index: 0,
            db,
            state,
            was_playing_before_background: false,
        }
    }

    /// Handle a user-dispatched AppAction.
    pub fn handle_action(
        &mut self,
        action: AppAction,
        runtime: &tokio::runtime::Runtime,
        core_tx: &Sender<CoreMsg>,
    ) {
        match action {
            AppAction::LoadText { text } => {
                self.stop_playback();
                let words = crate::core::parser::tokenize(&text);
                self.on_parse_complete(words);
            }

            AppAction::FileSelected { path } => {
                self.stop_playback();
                self.state.is_loading = true;
                self.state.error = None;
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let file_path = path.clone();
                let tx = core_tx.clone();
                runtime.spawn(async move {
                    let result = tokio::task::spawn_blocking(move || detect_and_parse(&path)).await;
                    match result {
                        Ok(Ok((words, file_hash))) => {
                            let _ = tx.send(CoreMsg::Internal(InternalEvent::ParseComplete {
                                words,
                                file_hash: Some(file_hash),
                                file_name: Some(file_name),
                                file_path: Some(file_path),
                            }));
                        }
                        Ok(Err(e)) => {
                            let _ = tx.send(CoreMsg::Internal(InternalEvent::ParseError {
                                message: e.to_string(),
                            }));
                        }
                        Err(e) => {
                            let _ = tx.send(CoreMsg::Internal(InternalEvent::ParseError {
                                message: format!("Parse task panicked: {e}"),
                            }));
                        }
                    }
                });
            }

            AppAction::Play => {
                if !self.words.is_empty() && !self.state.is_playing {
                    self.start_playback(runtime, core_tx);
                }
            }

            AppAction::Pause => {
                if self.state.is_playing {
                    self.stop_playback();
                    self.save_current_session();
                }
            }

            AppAction::Toggle => {
                if self.state.is_playing {
                    self.stop_playback();
                    self.save_current_session();
                } else if !self.words.is_empty() {
                    self.start_playback(runtime, core_tx);
                }
            }

            AppAction::SeekToProgress { percent } => {
                let was_playing = self.state.is_playing;
                if was_playing {
                    self.stop_playback();
                }
                let percent = percent.clamp(0.0, 100.0);
                let new_index = ((percent / 100.0) * self.state.total_words as f32) as u64;
                let new_index = new_index.min(self.state.total_words.saturating_sub(1));
                self.state.current_word_index = new_index;
                self.state.progress_percent = percent;
                self.update_display(new_index as usize);
                self.save_current_session();
                if was_playing && new_index < self.state.total_words {
                    self.playback_start_index = new_index;
                    self.start_playback(runtime, core_tx);
                }
            }

            AppAction::SetWPM { wpm } => {
                let wpm = wpm.clamp(100, 1000);
                let was_playing = self.state.is_playing;
                if was_playing {
                    self.stop_playback();
                }
                self.state.wpm = wpm;
                if was_playing {
                    self.playback_start_index = self.state.current_word_index;
                    self.start_playback(runtime, core_tx);
                }
            }

            AppAction::SetWordsPerGroup { n } => {
                let n = n.clamp(1, 5);
                self.state.words_per_group = n;
                self.update_display(self.state.current_word_index as usize);
            }

            AppAction::Replay => {
                self.stop_playback();
                self.state.current_word_index = 0;
                self.state.progress_percent = 0.0;
                self.playback_start_index = 0;
                self.update_display(0);
                self.start_playback(runtime, core_tx);
            }

            AppAction::PushScreen { screen } => {
                self.state.router.screen_stack.push(screen);
            }

            AppAction::PopScreen => {
                self.state.router.screen_stack.pop();
            }

            AppAction::ClearToast => {
                self.state.toast = None;
            }

            AppAction::ClearError => {
                self.state.error = None;
            }

            AppAction::BackgroundPause => {
                if self.state.is_playing {
                    self.was_playing_before_background = true;
                    self.stop_playback();
                    self.save_current_session();
                }
            }

            AppAction::Foregrounded => {
                if self.was_playing_before_background && !self.words.is_empty() {
                    self.was_playing_before_background = false;
                    self.playback_start_index = self.state.current_word_index;
                    self.start_playback(runtime, core_tx);
                } else {
                    self.was_playing_before_background = false;
                }
            }

            // Stub arms — wired in Plan 04 (history actor integration)
            AppAction::ResumeFile { file_hash: _ } => {}
            AppAction::DeleteSession { file_hash: _ } => {}
        }
    }

    /// Handle internal events from async tasks.
    pub fn handle_internal(
        &mut self,
        event: InternalEvent,
        runtime: &tokio::runtime::Runtime,
        core_tx: &Sender<CoreMsg>,
        update_tx: &Sender<AppUpdate>,
        shared_state: &Arc<RwLock<AppState>>,
    ) {
        match event {
            InternalEvent::ParseComplete { words, file_hash, file_name, file_path } => {
                // file_hash, file_name, file_path will be wired to on_parse_complete in Plan 04
                // For now accept the fields to satisfy the compiler; the _ prefix suppresses unused warnings
                let _ = (file_hash, file_name, file_path);
                self.on_parse_complete(words);
                emit(&mut self.state, shared_state, update_tx);
            }

            InternalEvent::ParseError { message } => {
                self.state.is_loading = false;
                self.state.error = Some(message);
                emit(&mut self.state, shared_state, update_tx);
            }

            InternalEvent::WordAdvance => {
                if !self.state.is_playing {
                    return;
                }
                let elapsed = self.playback_start.map(|s| s.elapsed()).unwrap_or_default();
                let new_index = compute_word_index(
                    elapsed,
                    self.state.wpm,
                    self.state.words_per_group,
                    self.state.total_words,
                ) + self.playback_start_index;
                let new_index = new_index.min(self.state.total_words.saturating_sub(1));

                let wpg = self.state.words_per_group.max(1) as u64;
                if new_index >= self.state.total_words.saturating_sub(wpg) {
                    // End of document
                    self.stop_playback();
                    self.state.current_word_index = self.state.total_words.saturating_sub(1);
                    self.state.progress_percent = 100.0;
                    self.update_display(self.state.current_word_index as usize);
                    emit(&mut self.state, shared_state, update_tx);
                    return;
                }

                self.state.current_word_index = new_index;
                let total = self.state.total_words as f32;
                self.state.progress_percent = if total > 0.0 {
                    (new_index as f32 / total) * 100.0
                } else {
                    0.0
                };
                let display = build_display(&self.words, new_index as usize, self.state.words_per_group as usize);
                self.state.display = Some(display.clone());

                // Update shared_state so desktop reads the new position via manager.state().
                // Without this, the desktop sees the frozen state from the Play-press emit
                // and only refreshes at end-of-document.
                self.state.rev += 1;
                match shared_state.write() {
                    Ok(mut g) => *g = self.state.clone(),
                    Err(p) => *p.into_inner() = self.state.clone(),
                }

                // Also emit granular PlaybackTick for reconcilers that handle it directly.
                let _ = update_tx.send(AppUpdate::PlaybackTick {
                    display,
                    progress_percent: self.state.progress_percent,
                    current_word_index: new_index,
                });
            }
        }
    }

    fn on_parse_complete(&mut self, words: Vec<String>) {
        self.state.is_loading = false;
        self.state.total_words = words.len() as u64;
        self.state.current_word_index = 0;
        self.state.progress_percent = 0.0;
        self.state.is_playing = false;
        self.playback_start_index = 0;

        // Check for session resume
        let text_hash = SessionData::compute_text_hash(&words.join(" "));
        if let Some(ref conn) = self.db {
            if let Ok(Some(session)) = restore_session(conn) {
                if session.text_hash == text_hash && session.word_index < words.len() as u64 {
                    self.state.current_word_index = session.word_index;
                    self.playback_start_index = session.word_index;
                    self.state.wpm = session.wpm;
                    self.state.words_per_group = session.words_per_group;
                }
            }
        }

        self.words = words;
        self.update_display(self.state.current_word_index as usize);
        self.state.router.screen_stack.push(Screen::Reading);
    }

    fn start_playback(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        core_tx: &Sender<CoreMsg>,
    ) {
        self.state.is_playing = true;
        self.playback_start = Some(Instant::now());
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        self.playback_cancel = Some(cancel_tx);

        let tx = core_tx.clone();
        let wpm = self.state.wpm;
        let interval_ms = (60_000u64 / wpm as u64).max(1);

        runtime.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            tokio::select! {
                _ = cancel_rx => {}
                _ = async {
                    loop {
                        interval.tick().await;
                        if tx.send(CoreMsg::Internal(InternalEvent::WordAdvance)).is_err() {
                            break;
                        }
                    }
                } => {}
            }
        });
    }

    fn stop_playback(&mut self) {
        // Drop cancel sender — closes oneshot — cancels timer task
        self.playback_cancel.take();
        self.state.is_playing = false;
        self.playback_start = None;
    }

    fn update_display(&mut self, word_start: usize) {
        if self.words.is_empty() {
            self.state.display = None;
            return;
        }
        let display = build_display(&self.words, word_start, self.state.words_per_group as usize);
        self.state.display = Some(display);
    }

    fn save_current_session(&self) {
        if let Some(ref conn) = self.db {
            if !self.words.is_empty() {
                let session = SessionData {
                    text_hash: SessionData::compute_text_hash(&self.words.join(" ")),
                    word_index: self.state.current_word_index,
                    wpm: self.state.wpm,
                    words_per_group: self.state.words_per_group,
                };
                save_session(conn, &session).ok();
            }
        }
    }
}

/// Build a WordDisplay from the word array at the given start index.
pub fn build_display(words: &[String], word_start: usize, words_per_group: usize) -> WordDisplay {
    let end = (word_start + words_per_group).min(words.len());
    let segments = words[word_start..end]
        .iter()
        .map(|w| compute_orp_anchor(w))
        .collect();
    WordDisplay { words: segments }
}

/// Compute the word-group start index from elapsed time and WPM.
/// Uses float arithmetic to self-correct for timer drift.
/// Per architecture bible §2.8 and reader.js loop() pattern.
pub fn compute_word_index(
    elapsed: Duration,
    wpm: u32,
    words_per_group: u32,
    total_words: u64,
) -> u64 {
    if total_words == 0 {
        return 0;
    }
    let wpg = words_per_group.max(1) as u64;
    let raw_words = (elapsed.as_secs_f64() * wpm as f64 / 60.0) as u64;
    let group = raw_words / wpg;
    let word_start = group * wpg;
    word_start.min(total_words.saturating_sub(wpg))
}

/// Write updated state to shared_state and send FullState update.
/// Called after every action except WordAdvance ticks.
/// Increments state.rev before cloning so every platform's update guard
/// (`if latest.rev > state.rev`) evaluates to true after the first dispatch.
pub fn emit(
    state: &mut AppState,
    shared_state: &Arc<RwLock<AppState>>,
    update_tx: &Sender<AppUpdate>,
) {
    state.rev += 1;
    let snapshot = state.clone();
    match shared_state.write() {
        Ok(mut g) => *g = snapshot.clone(),
        Err(p) => *p.into_inner() = snapshot.clone(),
    }
    let _ = update_tx.send(AppUpdate::FullState(snapshot));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use flume::unbounded;
    use crate::core::session::open_db;

    fn make_shared_history() -> Arc<RwLock<Vec<crate::state::HistoryEntry>>> {
        Arc::new(RwLock::new(vec![]))
    }

    // --- Task 1: Resume position tests (updated for file_sessions / new API) ---

    /// HIST-02: opening a file with an existing file_sessions entry restores word_index.
    #[test]
    fn test_resume_position_syncs_playback_start_index() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_resume_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let conn = open_db(&data_dir).unwrap();
        let words: Vec<String> = (0..100).map(|i| format!("word{i}")).collect();
        let file_hash = "a".repeat(64);
        // Pre-seed file_sessions at word_index=50
        conn.execute(
            "INSERT INTO file_sessions
             (file_hash, file_name, file_path, word_index, total_words, wpm, words_per_group, opened_at, updated_at)
             VALUES (?1, 'test.txt', '/tmp/test.txt', 50, 100, 300, 1, 1000, 1000)",
            rusqlite::params![file_hash],
        ).unwrap();
        drop(conn);

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        actor.on_parse_complete(
            words,
            Some(file_hash),
            Some("test.txt".to_string()),
            Some("/tmp/test.txt".to_string()),
        );

        assert_eq!(
            actor.playback_start_index, 50,
            "playback_start_index must equal file_sessions.word_index=50 after silent restore"
        );
        assert_eq!(actor.state.current_word_index, 50);
    }

    /// After on_parse_complete with no prior session, playback_start_index must be 0.
    #[test]
    fn test_no_session_playback_start_index_is_zero() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_nosession_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        let words: Vec<String> = (0..20).map(|i| format!("w{i}")).collect();
        actor.on_parse_complete(words, None, None, None);

        assert_eq!(actor.playback_start_index, 0);
    }

    /// When file_hash is new (not in file_sessions), word_index must start at 0.
    #[test]
    fn test_unknown_file_hash_starts_at_zero() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_unknown_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        let words: Vec<String> = (0..30).map(|i| format!("xyz{i}")).collect();
        actor.on_parse_complete(
            words,
            Some("b".repeat(64)),
            Some("unknown.txt".to_string()),
            Some("/tmp/unknown.txt".to_string()),
        );
        assert_eq!(actor.playback_start_index, 0);
        assert_eq!(actor.state.current_word_index, 0);
    }

    // --- Task 2: Background resume tests ---

    /// dispatch BackgroundPause while is_playing=true:
    /// was_playing_before_background must become true, is_playing must become false.
    #[test]
    fn test_background_pause_sets_flag() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_bgpause_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        // Load words and start playback manually
        actor.words = vec!["hello".to_string(), "world".to_string()];
        actor.state.total_words = 2;
        actor.state.is_playing = true;

        actor.handle_action(AppAction::BackgroundPause, &runtime, &core_tx);

        assert!(
            actor.was_playing_before_background,
            "was_playing_before_background must be true after BackgroundPause"
        );
        assert!(
            !actor.state.is_playing,
            "is_playing must be false after BackgroundPause"
        );
    }

    /// dispatch Pause (user-initiated) while playing:
    /// was_playing_before_background must remain false.
    #[test]
    fn test_user_pause_does_not_set_flag() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_userpause_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        actor.words = vec!["hello".to_string(), "world".to_string()];
        actor.state.total_words = 2;
        actor.state.is_playing = true;

        actor.handle_action(AppAction::Pause, &runtime, &core_tx);

        assert!(
            !actor.was_playing_before_background,
            "was_playing_before_background must remain false after user Pause"
        );
        assert!(
            !actor.state.is_playing,
            "is_playing must be false after Pause"
        );
    }

    /// dispatch Foregrounded when was_playing_before_background=true and words non-empty:
    /// is_playing must become true, flag must reset to false.
    #[test]
    fn test_foregrounded_resumes_playback() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_fgresume_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        actor.words = vec!["hello".to_string(), "world".to_string()];
        actor.state.total_words = 2;
        actor.was_playing_before_background = true;

        actor.handle_action(AppAction::Foregrounded, &runtime, &core_tx);

        assert!(
            actor.state.is_playing,
            "is_playing must be true after Foregrounded with was_playing_before_background=true"
        );
        assert!(
            !actor.was_playing_before_background,
            "was_playing_before_background must be reset to false after Foregrounded"
        );
    }

    /// dispatch Foregrounded when was_playing_before_background=true but words is empty:
    /// is_playing must remain false, flag must reset to false.
    #[test]
    fn test_foregrounded_no_words_stays_stopped() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_fgnowords_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        // words is empty (default)
        actor.was_playing_before_background = true;

        actor.handle_action(AppAction::Foregrounded, &runtime, &core_tx);

        assert!(
            !actor.state.is_playing,
            "is_playing must remain false when Foregrounded with empty word list"
        );
        assert!(
            !actor.was_playing_before_background,
            "was_playing_before_background must be reset to false"
        );
    }

    // --- Regression tests for words-per-group end-of-doc stuck bug ---

    /// compute_word_index must clamp to a value that the end-of-doc condition can reach.
    #[test]
    fn test_compute_word_index_max_does_not_exceed_last_group_start() {
        let very_long = Duration::from_secs(3600);

        let idx = compute_word_index(very_long, 300, 2, 5);
        assert_eq!(idx, 3, "last group start for wpg=2 total=5 should be 3, got {idx}");

        let idx = compute_word_index(very_long, 300, 2, 6);
        assert_eq!(idx, 4, "last group start for wpg=2 total=6 should be 4, got {idx}");

        let idx = compute_word_index(very_long, 300, 3, 7);
        assert_eq!(idx, 4, "last group start for wpg=3 total=7 should be 4, got {idx}");

        let idx = compute_word_index(very_long, 300, 1, 5);
        assert_eq!(idx, 4, "last group start for wpg=1 total=5 should be 4, got {idx}");
    }

    /// The end-of-doc condition in WordAdvance must fire when new_index reaches the
    /// last group start (total_words - wpg).
    #[test]
    fn test_word_advance_stops_at_end_of_doc_with_words_per_group_2() {
        use crate::actions::AppAction;
        use crate::updates::AppUpdate;

        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_{}", std::process::id()))
            .to_string_lossy()
            .to_string();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let (update_tx, update_rx) = flume::unbounded::<AppUpdate>();
        let shared = Arc::new(RwLock::new(crate::state::AppState::initial()));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());

        // Load 5 words (odd count)
        actor.handle_action(
            AppAction::LoadText {
                text: "one two three four five".to_string(),
            },
            &runtime,
            &core_tx,
        );
        // Drain any queued updates
        while update_rx.try_recv().is_ok() {}

        // Set words_per_group=2
        actor.handle_action(
            AppAction::SetWordsPerGroup { n: 2 },
            &runtime,
            &core_tx,
        );

        actor.state.is_playing = true;
        actor.state.total_words = 5;
        actor.playback_start_index = 0;
        actor.playback_start = Some(Instant::now() - Duration::from_secs(3600));

        actor.handle_internal(
            crate::updates::InternalEvent::WordAdvance,
            &runtime,
            &core_tx,
            &update_tx,
            &shared,
        );

        assert!(
            !actor.state.is_playing,
            "is_playing must be false after end-of-doc with words_per_group=2 (odd word count)"
        );
        assert_eq!(
            actor.state.progress_percent, 100.0,
            "progress_percent must be 100 at end-of-doc"
        );
    }

    #[test]
    fn test_emit_increments_rev() {
        let (update_tx, update_rx) = unbounded::<crate::updates::AppUpdate>();
        let shared = Arc::new(RwLock::new(crate::state::AppState::initial()));
        let mut state = crate::state::AppState::initial();

        emit(&mut state, &shared, &update_tx);
        assert_eq!(state.rev, 1, "rev must be 1 after first emit");

        emit(&mut state, &shared, &update_tx);
        assert_eq!(state.rev, 2, "rev must be 2 after second emit");

        let first = update_rx.try_recv().expect("first update missing");
        let second = update_rx.try_recv().expect("second update missing");
        match first {
            crate::updates::AppUpdate::FullState(s) => assert_eq!(s.rev, 1),
            _ => panic!("expected FullState"),
        }
        match second {
            crate::updates::AppUpdate::FullState(s) => assert_eq!(s.rev, 2),
            _ => panic!("expected FullState"),
        }
    }

    // --- Task 2 integration tests: HIST-01, HIST-02, HIST-03, STATE blocker ---

    /// HIST-01: opening a new file creates a file_sessions row.
    #[test]
    fn test_file_open_creates_history_entry() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_create_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();
        let mut actor = ActorState::new(&data_dir, make_shared_history());
        let words: Vec<String> = (0..50).map(|i| format!("w{i}")).collect();
        let hash = "c".repeat(64);
        actor.on_parse_complete(
            words,
            Some(hash.clone()),
            Some("book.epub".to_string()),
            Some("/tmp/book.epub".to_string()),
        );
        if let Some(ref conn) = actor.db {
            let found = crate::core::history::lookup_file_session(conn, &hash)
                .expect("lookup")
                .expect("must be Some");
            assert_eq!(found.file_name, "book.epub");
        } else {
            panic!("actor.db must be Some");
        }
    }

    /// HIST-03: LoadText (paste) must not create file_sessions rows.
    #[test]
    fn test_paste_does_not_create_history_entry() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_paste_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();
        let mut actor = ActorState::new(&data_dir, make_shared_history());
        let words: Vec<String> = vec!["hello".to_string(), "world".to_string()];
        actor.on_parse_complete(words, None, None, None);
        assert!(actor.current_file_hash.is_none(), "paste must not set current_file_hash");
        if let Some(ref conn) = actor.db {
            let history = crate::core::history::load_history(conn).expect("load");
            assert!(history.is_empty(), "paste must not create any file_sessions rows");
        }
    }

    /// STATE blocker: delete active session → next Pause must NOT re-insert.
    #[test]
    fn test_delete_active_session_prevents_reinsert() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_del_active_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        let words: Vec<String> = (0..100).map(|i| format!("w{i}")).collect();
        let hash = "d".repeat(64);

        // Open file — creates file_sessions row
        actor.on_parse_complete(
            words.clone(),
            Some(hash.clone()),
            Some("test.txt".to_string()),
            Some("/tmp/test.txt".to_string()),
        );
        actor.words = words;
        actor.state.total_words = 100;

        // Delete the active session
        actor.handle_action(AppAction::DeleteSession { file_hash: hash.clone() }, &runtime, &core_tx);

        // current_file_hash must be None
        assert!(actor.current_file_hash.is_none(), "current_file_hash must be None after deleting active session");

        // Pause — must not re-insert
        actor.handle_action(AppAction::Pause, &runtime, &core_tx);

        // Verify row is still gone
        if let Some(ref conn) = actor.db {
            let found = crate::core::history::lookup_file_session(conn, &hash).expect("lookup");
            assert!(found.is_none(), "deleted row must not be re-inserted after Pause");
        }
    }

    /// Deleting a different file's session must not clear current_file_hash.
    #[test]
    fn test_delete_inactive_session_does_not_clear_hash() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_del_inactive_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let (core_tx, _core_rx) = flume::unbounded::<crate::updates::CoreMsg>();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        let mut actor = ActorState::new(&data_dir, make_shared_history());
        let hash_a = "e".repeat(64);
        let hash_b = "f".repeat(64);

        // Open file A
        actor.on_parse_complete(
            vec!["one".to_string()],
            Some(hash_a.clone()),
            Some("a.txt".to_string()),
            Some("/tmp/a.txt".to_string()),
        );

        // Manually insert a row for file B so it can be deleted
        if let Some(ref conn) = actor.db {
            conn.execute(
                "INSERT INTO file_sessions
                 (file_hash, file_name, file_path, word_index, total_words, wpm, words_per_group, opened_at, updated_at)
                 VALUES (?1, 'b.txt', '/tmp/b.txt', 0, 10, 300, 1, 1000, 1000)",
                rusqlite::params![hash_b],
            ).unwrap();
        }

        // Delete file B
        actor.handle_action(AppAction::DeleteSession { file_hash: hash_b }, &runtime, &core_tx);

        // current_file_hash must still be Some(hash_a)
        assert_eq!(
            actor.current_file_hash.as_deref(),
            Some(hash_a.as_str()),
            "deleting a different session must not affect current_file_hash"
        );
    }
}
