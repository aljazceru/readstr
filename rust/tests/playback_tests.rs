//! Unit tests for playback timing logic.

use std::time::Duration;
use speedreading_app_core::core::actor::compute_word_index;

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
