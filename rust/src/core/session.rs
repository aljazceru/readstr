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
    // Migration v0 → v1: add file_sessions table for per-file reading history.
    // user_version = 0 means v1.0 schema (sessions table only, no file_sessions).
    // Safe to run on every startup — guard ensures it only executes once.
    let user_version: i32 = conn.query_row(
        "PRAGMA user_version",
        [],
        |r| r.get(0),
    )?;
    if user_version < 1 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS file_sessions (
                 file_hash       TEXT    PRIMARY KEY,
                 file_name       TEXT    NOT NULL,
                 file_path       TEXT    NOT NULL,
                 word_index      INTEGER NOT NULL DEFAULT 0,
                 total_words     INTEGER NOT NULL DEFAULT 0,
                 wpm             INTEGER NOT NULL DEFAULT 300,
                 words_per_group INTEGER NOT NULL DEFAULT 1,
                 opened_at       INTEGER NOT NULL DEFAULT 0,
                 updated_at      INTEGER NOT NULL DEFAULT 0
             );
             PRAGMA user_version = 1;
             COMMIT;",
        )
        .context("Failed to migrate schema to v1")?;
    }
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

    #[test]
    fn test_open_db_creates_file_sessions_table() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_migration_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();
        let conn = open_db(&data_dir).expect("open_db");
        // file_sessions table must exist after open_db
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='file_sessions'",
            [],
            |r| r.get(0),
        ).expect("query");
        assert_eq!(count, 1, "file_sessions table must exist after open_db");
    }

    #[test]
    fn test_open_db_migration_sets_user_version_1() {
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_uv_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();
        let conn = open_db(&data_dir).expect("open_db");
        let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).expect("pragma");
        assert_eq!(uv, 1, "user_version must be 1 after migration");
    }

    #[test]
    fn test_open_db_migration_idempotent() {
        // Simulate v1.0 device: create DB with only the sessions table (user_version=0)
        let data_dir = std::env::temp_dir()
            .join(format!("rmp_test_idem_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = std::path::Path::new(&data_dir).join("speedreading.db");
        {
            let conn = rusqlite::Connection::open(&db_path).expect("create v1.0 db");
            conn.execute_batch(
                "CREATE TABLE sessions (
                    id INTEGER PRIMARY KEY,
                    text_hash TEXT NOT NULL DEFAULT '',
                    word_index INTEGER NOT NULL DEFAULT 0,
                    wpm INTEGER NOT NULL DEFAULT 300,
                    words_per_group INTEGER NOT NULL DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT 0
                );"
            ).expect("v1.0 schema");
            // user_version stays 0 (default)
        }
        // Now open_db should run the migration exactly once
        let conn = open_db(&data_dir).expect("open_db on v1.0 db");
        let uv: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).expect("pragma");
        assert_eq!(uv, 1, "migration must run on v1.0 db");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='file_sessions'",
            [],
            |r| r.get(0),
        ).expect("query");
        assert_eq!(count, 1, "file_sessions must exist after migrating v1.0 db");
    }
}
