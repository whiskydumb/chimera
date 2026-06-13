use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::config::{Config, Paths};

/// opens the entry's content file in the user's editor and waits for it to exit.
/// returns whether the editor exited successfully.
pub fn edit(paths: &Paths, config: &Config, rel_path: &str) -> Result<bool> {
    let target = paths.library.join(rel_path);
    if !target.exists() {
        bail!("no such entry: {rel_path}");
    }
    let editor = resolve_editor(config);
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("editor command is empty")?;
    let status = Command::new(program)
        .args(parts)
        .arg(&target)
        .status()
        .with_context(|| format!("failed to launch editor '{program}'"))?;
    Ok(status.success())
}

/// resolves the editor command: config -> `$VISUAL` -> `$EDITOR` -> platform default.
fn resolve_editor(config: &Config) -> String {
    if let Some(editor) = config.editor.as_ref().filter(|e| !e.trim().is_empty()) {
        return editor.clone();
    }
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(val) = std::env::var(var)
            && !val.trim().is_empty()
        {
            return val;
        }
    }
    default_editor()
}

#[cfg(windows)]
fn default_editor() -> String {
    "notepad".to_string()
}

#[cfg(not(windows))]
fn default_editor() -> String {
    "vi".to_string()
}
