use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::config::Paths;

/// a row describing one indexed entry (metadata only, not its content).
#[derive(Debug, Clone)]
pub struct Record {
    pub id: String,
    pub rel_path: String,
    pub name: String,
    pub category: String,
    pub language: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
    pub sha256: String,
}

/// opens (creating if needed) the index database and applies the schema.
pub fn open(paths: &Paths) -> Result<Connection> {
    let conn = Connection::open(&paths.db)
        .with_context(|| format!("failed to open index at {}", paths.db.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS entries (
            id          TEXT PRIMARY KEY,
            rel_path    TEXT NOT NULL UNIQUE,
            name        TEXT NOT NULL,
            category    TEXT NOT NULL,
            language    TEXT,
            description TEXT NOT NULL DEFAULT '',
            tags        TEXT NOT NULL DEFAULT '',
            sha256      TEXT NOT NULL,
            size        INTEGER NOT NULL DEFAULT 0,
            mtime       INTEGER NOT NULL DEFAULT 0
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
            entry_id UNINDEXED,
            name,
            tags,
            description,
            content,
            tokenize = 'unicode61'
        );
        "#,
    )
    .context("failed to apply schema")?;
    Ok(())
}

/// removes every row from both the metadata and full-text tables.
pub fn clear(conn: &Connection) -> Result<()> {
    conn.execute_batch("DELETE FROM entries; DELETE FROM entries_fts;")
        .context("failed to clear index")?;
    Ok(())
}

/// inserts or replaces an entry (keyed by `rel_path`) and its full-text row.
pub fn upsert(conn: &Connection, rec: &Record, content: &str, size: u64, mtime: i64) -> Result<()> {
    let tags = rec.tags.join(",");
    conn.execute(
        r#"INSERT INTO entries
               (id, rel_path, name, category, language, description, tags, sha256, size, mtime)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
           ON CONFLICT(rel_path) DO UPDATE SET
               name        = excluded.name,
               category    = excluded.category,
               language    = excluded.language,
               description = excluded.description,
               tags        = excluded.tags,
               sha256      = excluded.sha256,
               size        = excluded.size,
               mtime       = excluded.mtime"#,
        params![
            rec.id,
            rec.rel_path,
            rec.name,
            rec.category,
            rec.language,
            rec.description,
            tags,
            rec.sha256,
            size as i64,
            mtime,
        ],
    )
    .context("failed to upsert entry")?;

    // rebuild the fts row for this entry (delete + insert keeps it simple/correct).
    conn.execute(
        "DELETE FROM entries_fts WHERE entry_id = ?1",
        params![rec.id],
    )?;
    conn.execute(
        "INSERT INTO entries_fts (entry_id, name, tags, description, content)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![rec.id, rec.name, tags, rec.description, content],
    )?;
    Ok(())
}

/// removes an entry (and its full-text row) by its library-relative path.
pub fn remove(conn: &Connection, rel_path: &str) -> Result<()> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM entries WHERE rel_path = ?1",
            params![rel_path],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = id {
        conn.execute("DELETE FROM entries_fts WHERE entry_id = ?1", params![id])?;
        conn.execute("DELETE FROM entries WHERE id = ?1", params![id])?;
    }
    Ok(())
}

/// returns entry ids matching a full-text query, best (lowest bm25) first.
pub fn search_fts(conn: &Connection, match_query: &str, limit: usize) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT entry_id FROM entries_fts
         WHERE entries_fts MATCH ?1
         ORDER BY bm25(entries_fts)
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![match_query, limit as i64], |row| {
        row.get::<_, String>(0)
    })?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

/// loads every entry's metadata (used for fuzzy/glob filtering and display).
pub fn all(conn: &Connection) -> Result<Vec<Record>> {
    let mut stmt = conn.prepare(
        "SELECT id, rel_path, name, category, language, description, tags, sha256 FROM entries",
    )?;
    let rows = stmt.query_map([], |row| {
        let tags: String = row.get(6)?;
        Ok(Record {
            id: row.get(0)?,
            rel_path: row.get(1)?,
            name: row.get(2)?,
            category: row.get(3)?,
            language: row.get(4)?,
            description: row.get(5)?,
            tags: if tags.is_empty() {
                Vec::new()
            } else {
                tags.split(',').map(str::to_string).collect()
            },
            sha256: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
