//! Integration test for session persistence using a real temp file DB.

use readstr_core::core::session::{open_db, restore_session, save_session, SessionData};
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("speedreading_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn test_session_roundtrip_file_db() {
    let dir = temp_dir();
    let conn = open_db(dir.to_str().unwrap()).expect("open_db");

    let session = SessionData {
        text_hash: SessionData::compute_text_hash("The quick brown fox"),
        word_index: 17,
        wpm: 350,
        words_per_group: 1,
    };

    save_session(&conn, &session).expect("save");
    let restored = restore_session(&conn).expect("restore").expect("some");

    assert_eq!(restored.word_index, 17);
    assert_eq!(restored.wpm, 350);
    assert_eq!(restored.text_hash, session.text_hash);

    // Cleanup
    std::fs::remove_dir_all(dir).ok();
}
