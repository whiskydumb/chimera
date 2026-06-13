use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use time::OffsetDateTime;

use crate::config::Paths;
use crate::library::entry::Sidecar;

/// copies the entry's content file into `dest_dir`, keeping its file name and,
/// on unix, its executable bit. refuses to overwrite an existing file.
pub fn copy_into_dir(paths: &Paths, rel_path: &str, dest_dir: &Path) -> Result<PathBuf> {
    let src = paths.library.join(rel_path);
    let file_name = src.file_name().context("entry has no file name")?;
    let dest = dest_dir.join(file_name);
    if dest.exists() {
        bail!("{} already exists; refusing to overwrite", dest.display());
    }
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("failed to create {}", dest_dir.display()))?;
    std::fs::copy(&src, &dest)
        .with_context(|| format!("failed to copy into {}", dest.display()))?;
    preserve_mode(&src, &dest)?;
    bump_uses(paths, rel_path)?;
    Ok(dest)
}

/// copies the entry's content to the system clipboard.
pub fn copy_to_clipboard(paths: &Paths, rel_path: &str) -> Result<()> {
    let src = paths.library.join(rel_path);
    let content = std::fs::read_to_string(&src)
        .with_context(|| format!("failed to read {}", src.display()))?;
    // @note: on x11/wayland arboard's clipboard ownership ends with the process,
    // so a value copied right before quitting may not persist there. it is fine
    // while the TUI stays open and on windows/macos; harden with fork/wait later.
    let mut clipboard = arboard::Clipboard::new().context("no system clipboard available")?;
    clipboard
        .set_text(content)
        .context("failed to set clipboard text")?;
    bump_uses(paths, rel_path)?;
    Ok(())
}

/// increments the entry's `uses` counter and bumps its `updated` timestamp.
fn bump_uses(paths: &Paths, rel_path: &str) -> Result<()> {
    let content = paths.library.join(rel_path);
    let sidecar_path = Sidecar::path_for(&content);
    if !sidecar_path.exists() {
        return Ok(());
    }
    let mut sidecar = Sidecar::load(&sidecar_path)?;
    sidecar.uses += 1;
    sidecar.updated = OffsetDateTime::now_utc();
    sidecar.save(&content)?;
    Ok(())
}

#[cfg(unix)]
fn preserve_mode(src: &Path, dest: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(src)?.permissions().mode();
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn preserve_mode(_src: &Path, _dest: &Path) -> Result<()> {
    Ok(())
}
