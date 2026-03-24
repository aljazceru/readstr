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

                if new_index >= self.state.total_words.saturating_sub(1) {
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

                // Emit granular PlaybackTick — avoids cloning full AppState per tick
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
