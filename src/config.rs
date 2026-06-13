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
    /// NORMAL-mode keyboard-layout map (vim `langmap`-style): space-separated
    /// `<layout-char><latin-key>` pairs. an empty string disables it.
    pub langmap: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor: None,
            theme: "base16-ocean.dark".to_string(),
            langmap: DEFAULT_LANGMAP.to_string(),
        }
    }
}

/// default NORMAL-mode layout map: the russian ЙЦУКЕН layout.
const DEFAULT_LANGMAP: &str =
    "йq цw уe кr еt нy гu шi щo зp фa ыs вd аf пg рh оj лk дl яz чx сc мv иb тn ьm";

/// the contents written by `chimera init`.
const DEFAULT_CONFIG: &str = "\
# chimera configuration.

# editor used by `chimera edit` and Ctrl-E in the TUI.
# falls back to $VISUAL / $EDITOR / the platform default when unset.
# editor = \"nvim\"

# syntax-highlighting theme for the preview. options:
# base16-ocean.dark, base16-ocean.light, base16-eighties.dark, base16-mocha.dark,
# nord, dracula, gruvbox-dark, gruvbox-light, solarized-dark, solarized-light,
# monokai, one-half-dark, one-half-light, catppuccin-mocha, catppuccin-macchiato,
# catppuccin-frappe, catppuccin-latte, github, inspired-github, coldark-dark,
# coldark-cold, dark-neon, sublime-snazzy, two-dark, zenburn, leet
theme = \"base16-ocean.dark\"

# vim NORMAL-mode keyboard-layout map (like vim's 'langmap'): space-separated
# \"<layout char><latin key>\" pairs. default maps the russian ЙЦУКЕН layout so the
# vim keys work on a russian layout; set \"\" to disable, or replace with your own.
# langmap = \"йq цw уe кr еt нy гu шi щo зp фa ыs вd аf пg рh оj лk дl яz чx сc мv иb тn ьm\"
";

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

    /// writes the default `config.toml`. returns `false` if it already exists and
    /// `force` is not set.
    pub fn write_default(paths: &Paths, force: bool) -> Result<bool> {
        if paths.config.exists() && !force {
            return Ok(false);
        }
        std::fs::write(&paths.config, DEFAULT_CONFIG)
            .with_context(|| format!("failed to write {}", paths.config.display()))?;
        Ok(true)
    }
}
