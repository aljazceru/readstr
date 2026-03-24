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

/// Regression test for: "pressing Play shows first word then immediately jumps to last word"
///
/// Root cause: WordAdvance (mid-document) sends PlaybackTick but does NOT update
/// shared_state. The desktop reads shared_state on every notification, so it always
/// sees stale data (index 0 from the Play press emit) until the end-of-document
/// emit fires.
///
/// This test proves: after a mid-document WordAdvance, shared_state.current_word_index
/// MUST equal the advanced index (not stale 0). If shared_state is NOT updated,
/// the test fails — demonstrating the bug before the fix.
#[test]
fn test_word_advance_tick_updates_shared_state() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let (core_tx, _core_rx) = unbounded::<CoreMsg>();
    let (update_tx, update_rx) = unbounded::<AppUpdate>();
    let shared_state = Arc::new(RwLock::new(speedreading_app_core::state::AppState::initial()));

    // 10-word document, playing from the start
    let words: Vec<String> = (1u32..=10).map(|i| format!("word{i}")).collect();
    let mut actor = ActorState::new("");
    actor.words = words;
    actor.state.total_words = 10;
    actor.state.wpm = 300;
    actor.state.words_per_group = 1;
    actor.state.is_playing = true;
    actor.state.current_word_index = 0;
    actor.playback_start_index = 0;
    // Simulate playback started 300ms ago → compute_word_index returns 1 (word #2)
    actor.playback_start = Some(
        std::time::Instant::now()
            .checked_sub(Duration::from_millis(300))
            .expect("instant subtraction"),
    );

    // Flush any initial shared_state updates first
    let initial_rev = shared_state.read().unwrap().rev;

    actor.handle_internal(
        InternalEvent::WordAdvance,
        &runtime,
        &core_tx,
        &update_tx,
        &shared_state,
    );

    // The update channel must have received a PlaybackTick (not FullState) for a mid-doc tick
    let update = update_rx.try_recv().expect("WordAdvance must send an update");
    match &update {
        AppUpdate::PlaybackTick { current_word_index, .. } => {
            assert_eq!(
                *current_word_index, 1,
                "PlaybackTick must carry advanced word index"
            );
        }
        AppUpdate::FullState(_) => {
            panic!("mid-document WordAdvance must send PlaybackTick, not FullState");
        }
    }

    // CRITICAL: shared_state must be updated so that the desktop can read the new position.
    // Without the fix, shared_state.current_word_index is still 0 here — this assertion fails.
    let snapped = shared_state.read().unwrap();
    assert_eq!(
        snapped.current_word_index, 1,
        "shared_state.current_word_index must be updated on WordAdvance so desktop reads correct position"
    );
    assert!(
        snapped.rev > initial_rev,
        "shared_state.rev must advance on WordAdvance tick"
    );
}
