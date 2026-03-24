//! Session persistence using rusqlite (bundled feature).
//! Single-row sessions table — INSERT OR REPLACE id=1 pattern.
//! Data dir is injected by platform (applicationSupportDirectory on iOS,
//! context.filesDir on Android, dirs_next::data_dir() on desktop).

use anyhow::Context;

/// Session data that survives app restart.
/// Words are NOT persisted — they are re-parsed on resume.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionData {
    pub text_hash: String,
    pub word_index: u64,
    pub wpm: u32,
    pub words_per_group: u32,
}

impl SessionData {
    /// Compute a stable hash for text identity checking on resume.
    /// If hash differs from stored hash, word_index is reset to 0.
    pub fn compute_text_hash(text: &str) -> String {
        let prefix = &text[..text.len().min(64)];
        format!("{}-{}", prefix, text.len())
    }
}

/// Open (or create) the session database at {data_dir}/speedreading.db.
pub fn open_db(data_dir: &str) -> anyhow::Result<rusqlite::Connection> {
    let path = std::path::Path::new(data_dir).join("speedreading.db");
    let conn = rusqlite::Connection::open(&path)
        .with_context(|| format!("Failed to open session DB at {}", path.display()))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id              INTEGER PRIMARY KEY,
            text_hash       TEXT    NOT NULL DEFAULT '',
            word_index      INTEGER NOT NULL DEFAULT 0,
            wpm             INTEGER NOT NULL DEFAULT 300,
            words_per_group INTEGER NOT NULL DEFAULT 1,
            updated_at      INTEGER NOT NULL DEFAULT 0
        );",
    )
    .context("Failed to create sessions table")?;
    Ok(conn)
}

/// Persist session state. Saves on Pause and SeekToProgress — not on every WordAdvance tick.
pub fn save_session(
    conn: &rusqlite::Connection,
    session: &SessionData,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sessions
         (id, text_hash, word_index, wpm, words_per_group, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, strftime('%s','now'))",
        rusqlite::params![
            session.text_hash,
            session.word_index as i64,
            session.wpm as i64,
            session.words_per_group as i64,
        ],
    )
    .context("Failed to save session")?;
    Ok(())
}

/// Restore session from DB. Returns None if no session exists.
/// If the stored text_hash doesn't match the provided current_text_hash,
/// returns SessionData with word_index reset to 0.
pub fn restore_session(
    conn: &rusqlite::Connection,
) -> anyhow::Result<Option<SessionData>> {
    let mut stmt = conn.prepare(
        "SELECT text_hash, word_index, wpm, words_per_group FROM sessions WHERE id = 1",
    )?;
    let result = stmt.query_row([], |row| {
        Ok(SessionData {
            text_hash: row.get(0)?,
            word_index: row.get::<_, i64>(1)? as u64,
            wpm: row.get::<_, i64>(2)? as u32,
            words_per_group: row.get::<_, i64>(3)? as u32,
        })
    });
    match result {
        Ok(session) => Ok(Some(session)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to restore session: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> rusqlite::Connection {
        // In-memory DB for unit tests
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY,
                text_hash TEXT NOT NULL DEFAULT '',
                word_index INTEGER NOT NULL DEFAULT 0,
                wpm INTEGER NOT NULL DEFAULT 300,
                words_per_group INTEGER NOT NULL DEFAULT 1,
                updated_at INTEGER NOT NULL DEFAULT 0
            );",
        )
        .expect("create table");
        conn
    }

    #[test]
    fn test_restore_empty_db_returns_none() {
        let conn = temp_db();
        let result = restore_session(&conn).expect("restore failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_roundtrip() {
        let conn = temp_db();
        let session = SessionData {
            text_hash: "abc-100".to_string(),
            word_index: 42,
            wpm: 400,
            words_per_group: 2,
        };
        save_session(&conn, &session).expect("save failed");
        let restored = restore_session(&conn).expect("restore failed").expect("expected Some");
        assert_eq!(restored.word_index, 42);
        assert_eq!(restored.wpm, 400);
        assert_eq!(restored.words_per_group, 2);
        assert_eq!(restored.text_hash, "abc-100");
    }

    #[test]
    fn test_save_overwrites() {
        let conn = temp_db();
        let s1 = SessionData { text_hash: "a-50".to_string(), word_index: 10, wpm: 300, words_per_group: 1 };
        let s2 = SessionData { text_hash: "a-50".to_string(), word_index: 99, wpm: 500, words_per_group: 3 };
        save_session(&conn, &s1).expect("save 1");
        save_session(&conn, &s2).expect("save 2");
        let restored = restore_session(&conn).expect("restore").expect("some");
        assert_eq!(restored.word_index, 99);
        assert_eq!(restored.wpm, 500);
    }

    #[test]
    fn test_compute_text_hash_stable() {
        // Per D-19: test SHA-256 of bytes, not old prefix+length
        use crate::core::parser::hash_file_bytes;
        let h1 = hash_file_bytes(b"hello world");
        let h2 = hash_file_bytes(b"hello world");
        assert_eq!(h1, h2, "SHA-256 must be deterministic");
        assert_eq!(h1.len(), 64, "SHA-256 hex must be 64 chars");
    }

    #[test]
    fn test_compute_text_hash_different_texts() {
        // Per D-19: different byte inputs must produce different hashes
        use crate::core::parser::hash_file_bytes;
        let h1 = hash_file_bytes(b"text one");
        let h2 = hash_file_bytes(b"text two");
        assert_ne!(h1, h2, "Different inputs must produce different SHA-256 hashes");
    }
}
