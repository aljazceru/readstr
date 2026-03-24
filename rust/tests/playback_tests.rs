//! Unit tests for playback timing logic.

use std::sync::{Arc, RwLock};
use std::time::Duration;
use flume::unbounded;
use speedreading_app_core::core::actor::{ActorState, compute_word_index};
use speedreading_app_core::updates::{AppUpdate, CoreMsg, InternalEvent};

#[test]
fn test_compute_word_index_basic() {
    // 300ms elapsed at 300wpm with 1 wpg in a 100-word document
    // raw = 0.3 * 300/60 = 1.5 → floor = 1
    let idx = compute_word_index(Duration::from_millis(300), 300, 1, 100);
    assert_eq!(idx, 1);
}

#[test]
fn test_compute_word_index_zero_elapsed() {
    let idx = compute_word_index(Duration::from_millis(0), 300, 1, 100);
    assert_eq!(idx, 0);
}

#[test]
fn test_compute_word_index_clamped_at_end() {
    // 10s elapsed at 300wpm = 50 words, but document only has 10 words
    let idx = compute_word_index(Duration::from_secs(10), 300, 1, 10);
    assert_eq!(idx, 9, "should clamp to total_words - 1");
}

#[test]
fn test_compute_word_index_with_words_per_group() {
    // 1s elapsed at 300wpm = 5 raw words; with wpg=2: group=2, word_start=4
    let idx = compute_word_index(Duration::from_secs(1), 300, 2, 100);
    assert_eq!(idx, 4);
}

#[test]
fn test_compute_word_index_empty_document() {
    let idx = compute_word_index(Duration::from_secs(5), 300, 1, 0);
    assert_eq!(idx, 0);
}

#[test]
fn test_drift_self_correction() {
    // A late tick (200% of expected interval) should still advance by ~2 positions
    // 400ms at 300wpm = 2 words (exactly 2.0 raw)
    let idx = compute_word_index(Duration::from_millis(400), 300, 1, 100);
    assert_eq!(idx, 2);
}

#[test]
fn test_actor_end_of_document_stop() {
    // SC-4: end-of-document stop exercised at the actor lifecycle level (not just unit math).
    //
    // Setup: ActorState with 5 words, playback started at word index 4 (last word).
    // Simulate a WordAdvance tick where elapsed time is ~0 so compute_word_index returns 0,
    // but playback_start_index = 4 (total_words - 1), so new_index = 4 = total_words - 1.
    // handle_internal must detect this as end-of-doc and set is_playing = false.

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let (core_tx, _core_rx) = unbounded::<CoreMsg>();
    let (update_tx, _update_rx) = unbounded::<AppUpdate>();
    let shared_state = Arc::new(RwLock::new(speedreading_app_core::state::AppState::initial()));

    // Create actor with 5 words loaded manually (bypass DB by using "")
    let mut actor = ActorState::new("");
    actor.words = vec![
        "one".to_string(),
        "two".to_string(),
        "three".to_string(),
        "four".to_string(),
        "five".to_string(),
    ];
    actor.state.total_words = 5;
    actor.state.is_playing = true;
    actor.state.current_word_index = 0;
    // Set playback_start_index to last word so that even with elapsed~0,
    // new_index = compute_word_index(~0) + 4 = 0 + 4 = 4 = total_words - 1
    actor.playback_start_index = 4;
    actor.playback_start = Some(std::time::Instant::now());

    // Dispatch WordAdvance — this should detect end-of-doc and stop playback
    actor.handle_internal(
        InternalEvent::WordAdvance,
        &runtime,
        &core_tx,
        &update_tx,
        &shared_state,
    );

    assert!(
        !actor.state.is_playing,
        "is_playing must be false after reaching end-of-document"
    );
    assert_eq!(
        actor.state.current_word_index, 4,
        "current_word_index must be total_words - 1 = 4 at end-of-document"
    );
    assert_eq!(
        actor.state.progress_percent, 100.0,
        "progress_percent must be 100.0 at end-of-document"
    );
}
