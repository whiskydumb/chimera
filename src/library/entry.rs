use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// suffix appended to a content file to form its metadata sidecar,
/// e.g. `deploy.sh` -> `deploy.sh.chm.toml`.
pub const SIDECAR_SUFFIX: &str = ".chm.toml";

/// metadata persisted next to each content file.
///
/// @note: the content file itself stays byte-for-byte untouched so it can be
/// reused as-is; everything chimera knows about an entry lives here. this makes
/// the sqlite index a pure, rebuildable cache rather than a source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sidecar {
    /// stable identifier, independent of the file name or path.
    pub id: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: String,
    /// language override; inferred from the extension when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// original location the entry was imported from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub added: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    /// how many times this entry has been reused.
    #[serde(default)]
    pub uses: u64,
    /// sha256 of the content file; used for change detection and dedup.
    pub sha256: String,
}

impl Sidecar {
    /// returns the sidecar path for a given content file.
    pub fn path_for(content: &Path) -> PathBuf {
        let mut name = content.as_os_str().to_owned();
        name.push(SIDECAR_SUFFIX);
        PathBuf::from(name)
    }

    /// loads a sidecar from disk.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read sidecar {}", path.display()))?;
        let sidecar = toml::from_str(&raw)
            .with_context(|| format!("failed to parse sidecar {}", path.display()))?;
        Ok(sidecar)
    }

    /// writes the sidecar next to the given content file.
    pub fn save(&self, content: &Path) -> Result<()> {
        let path = Self::path_for(content);
        let raw = toml::to_string(self).context("failed to serialize sidecar")?;
        std::fs::write(&path, raw)
            .with_context(|| format!("failed to write sidecar {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_for_appends_suffix() {
        let p = Sidecar::path_for(Path::new("a/b/deploy.sh"));
        assert!(p.to_string_lossy().ends_with("deploy.sh.chm.toml"));
    }

    #[test]
    fn sidecar_round_trips_on_disk() {
        let dir = std::env::temp_dir().join(format!("chimera_entry_{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        let content = dir.join("snippet.sh");
        std::fs::write(&content, "echo hi").unwrap();

        let now = OffsetDateTime::now_utc();
        let sidecar = Sidecar {
            id: "01ABC".to_string(),
            tags: vec!["a".to_string(), "b".to_string()],
            description: "desc".to_string(),
            language: None,
            source: Some("/orig".to_string()),
            added: now,
            updated: now,
            uses: 3,
            sha256: "deadbeef".to_string(),
        };
        sidecar.save(&content).unwrap();
        let loaded = Sidecar::load(&Sidecar::path_for(&content)).unwrap();

        assert_eq!(loaded.id, "01ABC");
        assert_eq!(loaded.tags, vec!["a", "b"]);
        assert_eq!(loaded.uses, 3);
        assert_eq!(loaded.sha256, "deadbeef");
        assert_eq!(loaded.added, now);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
