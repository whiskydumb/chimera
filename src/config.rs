use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

const ENV_HOME: &str = "CHIMERA_HOME";
const CONFIG_FILE: &str = "config.toml";
const DB_FILE: &str = "index.sqlite";
const LIBRARY_DIR: &str = "library";

/// resolved filesystem locations for a chimera library.
#[derive(Debug, Clone)]
pub struct Paths {
    // @note: `root` is currently only referenced by tests.
    #[allow(dead_code)]
    pub root: PathBuf,
    pub library: PathBuf,
    pub db: PathBuf,
    pub config: PathBuf,
}

impl Paths {
    /// resolves the chimera root: `$CHIMERA_HOME` when set, otherwise `~/.chimera`.
    pub fn resolve() -> Result<Self> {
        let root = match std::env::var_os(ENV_HOME) {
            Some(val) => PathBuf::from(val),
            None => {
                let dirs = directories::BaseDirs::new()
                    .context("could not determine the home directory")?;
                dirs.home_dir().join(".chimera")
            }
        };
        Ok(Self {
            library: root.join(LIBRARY_DIR),
            db: root.join(DB_FILE),
            config: root.join(CONFIG_FILE),
            root,
        })
    }

    /// creates the root and library directories if they are missing.
    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.library).with_context(|| {
            format!("failed to create library dir at {}", self.library.display())
        })?;
        Ok(())
    }
}

/// user-tunable settings stored in `config.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// editor command; falls back to `$VISUAL` / `$EDITOR` / platform default when empty.
    pub editor: Option<String>,
    /// syntax-highlighting theme (a syntect default theme name).
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor: None,
            theme: "base16-ocean.dark".to_string(),
        }
    }
}

impl Config {
    /// loads `config.toml`, returning defaults when the file is absent.
    pub fn load(paths: &Paths) -> Result<Self> {
        if !paths.config.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&paths.config)
            .with_context(|| format!("failed to read {}", paths.config.display()))?;
        toml::from_str(&raw).with_context(|| format!("failed to parse {}", paths.config.display()))
    }
}
