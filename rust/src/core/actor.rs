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
                let tx = core_tx.clone();
                runtime.spawn(async move {
                    let result = tokio::task::spawn_blocking(move || detect_and_parse(&path)).await;
                    match result {
                        Ok(Ok(words)) => {
                            let _ = tx.send(CoreMsg::Internal(InternalEvent::ParseComplete { words }));
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

            AppAction::Foregrounded => {
                // No-op in Phase 1 — mobile lifecycle handled in Phase 3/4
            }
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
            InternalEvent::ParseComplete { words } => {
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
    use crate::core::session::{open_db, save_session, SessionData};

    // --- Task 1: Resume position tests ---

    /// After on_parse_complete with a matching session (word_index=50),
    /// actor.playback_start_index must equal session.word_index.
    #[test]
    fn test_resume_position_syncs_playback_start_index() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_resume_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        // Pre-seed the DB with a session at word_index=50
        let conn = open_db(&data_dir).unwrap();
        let words: Vec<String> = (0..100).map(|i| format!("word{i}")).collect();
        let text_hash = SessionData::compute_text_hash(&words.join(" "));
        save_session(&conn, &SessionData {
            text_hash,
            word_index: 50,
            wpm: 300,
            words_per_group: 1,
        }).unwrap();
        drop(conn);

        let mut actor = ActorState::new(&data_dir);
        actor.on_parse_complete(words);

        assert_eq!(
            actor.playback_start_index, 50,
            "playback_start_index must equal session.word_index=50 after session restore"
        );
    }

    /// After on_parse_complete with no prior session, playback_start_index must be 0.
    #[test]
    fn test_no_session_playback_start_index_is_zero() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_nosession_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        let mut actor = ActorState::new(&data_dir);
        let words: Vec<String> = (0..20).map(|i| format!("w{i}")).collect();
        actor.on_parse_complete(words);

        assert_eq!(
            actor.playback_start_index, 0,
            "playback_start_index must be 0 when no session exists"
        );
    }

    /// After on_parse_complete with a mismatched text_hash, playback_start_index must be 0.
    #[test]
    fn test_hash_mismatch_playback_start_index_is_zero() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_hashmismatch_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();

        // Save a session with a different text_hash
        let conn = open_db(&data_dir).unwrap();
        save_session(&conn, &SessionData {
            text_hash: "different-hash-999".to_string(),
            word_index: 75,
            wpm: 300,
            words_per_group: 1,
        }).unwrap();
        drop(conn);

        let mut actor = ActorState::new(&data_dir);
        // These words produce a different hash than "different-hash-999"
        let words: Vec<String> = (0..30).map(|i| format!("xyz{i}")).collect();
        actor.on_parse_complete(words);

        assert_eq!(
            actor.playback_start_index, 0,
            "playback_start_index must be 0 when text hash does not match stored session"
        );
    }

    // --- Regression tests for words-per-group end-of-doc stuck bug ---

    /// compute_word_index must clamp to a value that the end-of-doc condition can reach.
    /// For wpg=2, total_words=5: last group starts at index 3 (words 3+4).
    /// The end-of-doc check must fire when new_index == 3, not require index 4 (which
    /// compute_word_index never returns when wpg=2).
    #[test]
    fn test_compute_word_index_max_does_not_exceed_last_group_start() {
        // Simulate a long elapsed time — index should saturate at total_words - wpg.
        let very_long = Duration::from_secs(3600);

        // odd total_words, wpg=2
        let idx = compute_word_index(very_long, 300, 2, 5);
        assert_eq!(idx, 3, "last group start for wpg=2 total=5 should be 3, got {idx}");

        // even total_words, wpg=2
        let idx = compute_word_index(very_long, 300, 2, 6);
        assert_eq!(idx, 4, "last group start for wpg=2 total=6 should be 4, got {idx}");

        // wpg=3, total=7: last full group starts at index 4 (covers words 4,5,6)
        let idx = compute_word_index(very_long, 300, 3, 7);
        assert_eq!(idx, 4, "last group start for wpg=3 total=7 should be 4, got {idx}");

        // wpg=1 still works
        let idx = compute_word_index(very_long, 300, 1, 5);
        assert_eq!(idx, 4, "last group start for wpg=1 total=5 should be 4, got {idx}");
    }

    /// The end-of-doc condition in WordAdvance must fire when new_index reaches the
    /// last group start (total_words - wpg), not just when it reaches total_words - 1.
    ///
    /// This test drives an ActorState through the WordAdvance handler and confirms
    /// is_playing becomes false after playback reaches the last group.
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

        let mut actor = ActorState::new(&data_dir);

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

        // Manually set playback state as if we're at the last group start (index 3).
        // This simulates the actor receiving a WordAdvance tick after playing up to that point.
        actor.state.is_playing = true;
        actor.state.total_words = 5;
        actor.playback_start_index = 0;
        // Set playback_start to a time far enough in the past that compute_word_index saturates.
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

        // Verify updates were sent
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
}
