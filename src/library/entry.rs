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
