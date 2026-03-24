//! Per-file reading history persistence using the file_sessions table.
//! All functions accept a &rusqlite::Connection — the actor owns the Connection.
//! Errors are returned as anyhow::Result; call sites suppress with .ok() (same pattern as session.rs).

use anyhow::Context;

/// Internal row type — used by all history functions.
/// Not exported via FFI; the FFI type HistoryEntry (in state.rs) is built from this.
#[derive(Debug, Clone)]
pub struct FileSessionRow {
    pub file_hash: String,
    pub file_name: String,
    pub file_path: String,
    pub word_index: u64,
    pub total_words: u64,
    pub wpm: u32,
    pub words_per_group: u32,
    pub opened_at: i64,
    pub updated_at: i64,
}

impl FileSessionRow {
    /// Pre-computed progress percentage for FFI layer convenience.
    pub fn progress_percent(&self) -> f32 {
        if self.total_words == 0 {
            0.0
        } else {
            self.word_index as f32 / self.total_words as f32 * 100.0
        }
    }
}

/// Insert or replace a file session row.
/// opened_at is preserved on subsequent calls via COALESCE — it reflects first open, not last progress save.
/// updated_at is always set to the current Unix epoch second.
pub fn upsert_file_session(conn: &rusqlite::Connection, row: &FileSessionRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO file_sessions
         (file_hash, file_name, file_path, word_index, total_words,
          wpm, words_per_group, opened_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 COALESCE(
                     (SELECT opened_at FROM file_sessions WHERE file_hash = ?1),
                     strftime('%s','now')
                 ),
                 strftime('%s','now'))",
        rusqlite::params![
            row.file_hash,
            row.file_name,
            row.file_path,
            row.word_index as i64,
            row.total_words as i64,
            row.wpm as i64,
            row.words_per_group as i64,
        ],
    )
    .context("Failed to upsert file session")?;
    Ok(())
}

/// Update progress fields only (word_index, total_words, wpm, words_per_group, updated_at).
/// Does NOT touch opened_at or file_name/file_path.
/// No-op if the row does not exist (hash not in file_sessions).
pub fn update_progress(
    conn: &rusqlite::Connection,
    file_hash: &str,
    word_index: u64,
    total_words: u64,
    wpm: u32,
    words_per_group: u32,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE file_sessions
         SET word_index = ?2, total_words = ?3, wpm = ?4, words_per_group = ?5,
             updated_at = strftime('%s','now')
         WHERE file_hash = ?1",
        rusqlite::params![
            file_hash,
            word_index as i64,
            total_words as i64,
            wpm as i64,
            words_per_group as i64,
        ],
    )
    .context("Failed to update progress")?;
    Ok(())
}

/// Load all history rows ordered by opened_at descending (most recently opened first).
pub fn load_history(conn: &rusqlite::Connection) -> anyhow::Result<Vec<FileSessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, file_name, file_path, word_index, total_words,
                wpm, words_per_group, opened_at, updated_at
         FROM file_sessions
         ORDER BY opened_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FileSessionRow {
                file_hash: row.get(0)?,
                file_name: row.get(1)?,
                file_path: row.get(2)?,
                word_index: row.get::<_, i64>(3)? as u64,
                total_words: row.get::<_, i64>(4)? as u64,
                wpm: row.get::<_, i64>(5)? as u32,
                words_per_group: row.get::<_, i64>(6)? as u32,
                opened_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to load history")?;
    Ok(rows)
}

/// Look up a single file session by hash. Returns None if not found.
pub fn lookup_file_session(
    conn: &rusqlite::Connection,
    file_hash: &str,
) -> anyhow::Result<Option<FileSessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_hash, file_name, file_path, word_index, total_words,
                wpm, words_per_group, opened_at, updated_at
         FROM file_sessions WHERE file_hash = ?1",
    )?;
    let result = stmt.query_row(rusqlite::params![file_hash], |row| {
        Ok(FileSessionRow {
            file_hash: row.get(0)?,
            file_name: row.get(1)?,
            file_path: row.get(2)?,
            word_index: row.get::<_, i64>(3)? as u64,
            total_words: row.get::<_, i64>(4)? as u64,
            wpm: row.get::<_, i64>(5)? as u32,
            words_per_group: row.get::<_, i64>(6)? as u32,
            opened_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    });
    match result {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to lookup file session: {e}")),
    }
}

/// Delete a file session by hash. No-op if not found.
pub fn delete_file_session(conn: &rusqlite::Connection, file_hash: &str) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM file_sessions WHERE file_hash = ?1",
        rusqlite::params![file_hash],
    )
    .context("Failed to delete file session")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS file_sessions (
                file_hash       TEXT    PRIMARY KEY,
                file_name       TEXT    NOT NULL,
                file_path       TEXT    NOT NULL,
                word_index      INTEGER NOT NULL DEFAULT 0,
                total_words     INTEGER NOT NULL DEFAULT 0,
                wpm             INTEGER NOT NULL DEFAULT 300,
                words_per_group INTEGER NOT NULL DEFAULT 1,
                opened_at       INTEGER NOT NULL DEFAULT 0,
                updated_at      INTEGER NOT NULL DEFAULT 0
            );",
        )
        .expect("create file_sessions");
        conn
    }

    fn sample_row(hash: &str) -> FileSessionRow {
        FileSessionRow {
            file_hash: hash.to_string(),
            file_name: "test.epub".to_string(),
            file_path: "/tmp/test.epub".to_string(),
            word_index: 10,
            total_words: 100,
            wpm: 300,
            words_per_group: 1,
            opened_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_upsert_and_lookup() {
        let conn = temp_db();
        let row = sample_row("abc123");
        upsert_file_session(&conn, &row).expect("upsert");
        let found = lookup_file_session(&conn, "abc123").expect("lookup").expect("Some");
        assert_eq!(found.file_hash, "abc123");
        assert_eq!(found.word_index, 10);
        assert_eq!(found.total_words, 100);
    }

    #[test]
    fn test_lookup_missing_returns_none() {
        let conn = temp_db();
        let result = lookup_file_session(&conn, "nonexistent").expect("lookup");
        assert!(result.is_none());
    }

    #[test]
    fn test_upsert_preserves_opened_at() {
        let conn = temp_db();
        // Insert row with explicit opened_at=0 (from COALESCE fallback to strftime)
        // Then upsert again and check opened_at did not change
        let row = sample_row("hash_preserve");
        upsert_file_session(&conn, &row).expect("first upsert");
        let first = lookup_file_session(&conn, "hash_preserve").expect("lookup").expect("Some");
        let first_opened = first.opened_at;

        // Second upsert with different word_index
        let mut row2 = sample_row("hash_preserve");
        row2.word_index = 50;
        upsert_file_session(&conn, &row2).expect("second upsert");
        let second = lookup_file_session(&conn, "hash_preserve").expect("lookup").expect("Some");

        assert_eq!(second.opened_at, first_opened, "opened_at must not change on second upsert");
        assert_eq!(second.word_index, 50, "word_index must be updated");
    }

    #[test]
    fn test_update_progress() {
        let conn = temp_db();
        let row = sample_row("hash_progress");
        upsert_file_session(&conn, &row).expect("upsert");
        update_progress(&conn, "hash_progress", 75, 200, 400, 2).expect("update");
        let found = lookup_file_session(&conn, "hash_progress").expect("lookup").expect("Some");
        assert_eq!(found.word_index, 75);
        assert_eq!(found.total_words, 200);
        assert_eq!(found.wpm, 400);
        assert_eq!(found.words_per_group, 2);
    }

    #[test]
    fn test_delete_file_session() {
        let conn = temp_db();
        let row = sample_row("hash_delete");
        upsert_file_session(&conn, &row).expect("upsert");
        delete_file_session(&conn, "hash_delete").expect("delete");
        let result = lookup_file_session(&conn, "hash_delete").expect("lookup");
        assert!(result.is_none(), "row must be gone after delete");
    }

    #[test]
    fn test_load_history_ordered_by_opened_at_desc() {
        let conn = temp_db();
        // Insert two rows with different opened_at via direct SQL to control timing
        conn.execute_batch(
            "INSERT INTO file_sessions
             (file_hash, file_name, file_path, word_index, total_words, wpm, words_per_group, opened_at, updated_at)
             VALUES ('hash_old', 'old.txt', '/tmp/old.txt', 0, 50, 300, 1, 1000, 1000);
             INSERT INTO file_sessions
             (file_hash, file_name, file_path, word_index, total_words, wpm, words_per_group, opened_at, updated_at)
             VALUES ('hash_new', 'new.txt', '/tmp/new.txt', 0, 50, 300, 1, 2000, 2000);",
        ).expect("seed");
        let history = load_history(&conn).expect("load");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].file_hash, "hash_new", "most recent first");
        assert_eq!(history[1].file_hash, "hash_old");
    }

    #[test]
    fn test_progress_percent() {
        let row = FileSessionRow {
            file_hash: String::new(), file_name: String::new(), file_path: String::new(),
            word_index: 50, total_words: 200, wpm: 300, words_per_group: 1,
            opened_at: 0, updated_at: 0,
        };
        assert_eq!(row.progress_percent(), 25.0);
    }

    #[test]
    fn test_progress_percent_zero_total() {
        let row = FileSessionRow {
            file_hash: String::new(), file_name: String::new(), file_path: String::new(),
            word_index: 0, total_words: 0, wpm: 300, words_per_group: 1,
            opened_at: 0, updated_at: 0,
        };
        assert_eq!(row.progress_percent(), 0.0);
    }
}
