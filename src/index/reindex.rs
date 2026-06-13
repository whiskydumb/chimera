use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use time::OffsetDateTime;
use ulid::Ulid;

use crate::config::Paths;
use crate::index::db::{self, Record};
use crate::library::entry::{SIDECAR_SUFFIX, Sidecar};
use crate::library::store;

/// clears the index and rebuilds it by walking the library. returns the count.
pub fn rebuild(conn: &Connection, paths: &Paths) -> Result<usize> {
    db::clear(conn)?;
    if !paths.library.exists() {
        return Ok(0);
    }
    let walker = ignore::WalkBuilder::new(&paths.library)
        .hidden(false)
        .git_ignore(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut count = 0usize;
    for entry in walker {
        let entry = entry.context("error while walking the library")?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if path.to_string_lossy().ends_with(SIDECAR_SUFFIX) {
            continue;
        }
        if index_one(conn, paths, path)? {
            count += 1;
        }
    }
    tracing::info!(count, "library reindexed");
    Ok(count)
}

/// indexes a single content file. returns `false` when the file was skipped
/// (e.g. it turned out to be binary). synthesizes a sidecar if one is missing.
pub(crate) fn index_one(conn: &Connection, paths: &Paths, path: &Path) -> Result<bool> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.contains(&0) {
        return Ok(false);
    }
    let content = String::from_utf8_lossy(&bytes).into_owned();

    let meta = std::fs::metadata(path)?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let sidecar_path = Sidecar::path_for(path);
    let sidecar = if sidecar_path.exists() {
        Sidecar::load(&sidecar_path)?
    } else {
        // file added to the library outside chimera: synthesize and persist a sidecar.
        let now = OffsetDateTime::now_utc();
        let sidecar = Sidecar {
            id: Ulid::new().to_string(),
            tags: Vec::new(),
            description: String::new(),
            language: None,
            source: None,
            added: now,
            updated: now,
            uses: 0,
            sha256: store::sha256_hex(&bytes),
        };
        sidecar.save(path)?;
        sidecar
    };

    let rel = path
        .strip_prefix(&paths.library)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let category = rel.split('/').next().unwrap_or("misc").to_string();

    let record = Record {
        id: sidecar.id,
        rel_path: rel,
        name,
        category,
        language: sidecar.language,
        description: sidecar.description,
        tags: sidecar.tags,
        sha256: sidecar.sha256,
    };
    db::upsert(conn, &record, &content, size, mtime)?;
    tracing::debug!(path = %path.display(), "indexed entry");
    Ok(true)
}
