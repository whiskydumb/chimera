use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use time::OffsetDateTime;
use ulid::Ulid;

use crate::config::Paths;
use crate::library::entry::{SIDECAR_SUFFIX, Sidecar};

/// options controlling how an entry is added.
#[derive(Debug, Default, Clone)]
pub struct AddOptions {
    /// destination category (sub-directory); inferred from the extension when `None`.
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
}

/// the outcome of adding a single file.
#[derive(Debug)]
pub struct Added {
    pub abs_path: PathBuf,
    pub rel_path: String,
    /// true when an identical file was already present and nothing was copied.
    pub deduped: bool,
}

/// adds a file, or a directory recursively, to the library.
pub fn add(paths: &Paths, input: &Path, opts: &AddOptions) -> Result<Vec<Added>> {
    let meta =
        std::fs::metadata(input).with_context(|| format!("cannot stat {}", input.display()))?;
    let mut added = Vec::new();
    if meta.is_dir() {
        add_dir(paths, input, opts, &mut added)?;
    } else if meta.is_file() {
        if is_binary(input)? {
            bail!(
                "{} looks like a binary file; chimera stores text artifacts",
                input.display()
            );
        }
        if let Some(a) = add_file(paths, input, opts)? {
            added.push(a);
        }
    } else {
        bail!("{} is neither a file nor a directory", input.display());
    }
    Ok(added)
}

fn add_dir(paths: &Paths, dir: &Path, opts: &AddOptions, out: &mut Vec<Added>) -> Result<()> {
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker {
        let entry = entry.context("error while walking directory")?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if path.to_string_lossy().ends_with(SIDECAR_SUFFIX) {
            continue;
        }
        if is_binary(path)? {
            continue;
        }
        if let Some(a) = add_file(paths, path, opts)? {
            out.push(a);
        }
    }
    Ok(())
}

fn add_file(paths: &Paths, src: &Path, opts: &AddOptions) -> Result<Option<Added>> {
    let bytes = std::fs::read(src).with_context(|| format!("failed to read {}", src.display()))?;
    if is_binary_bytes(&bytes) {
        return Ok(None);
    }
    let sha = sha256_hex(&bytes);

    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .context("source has no valid utf-8 file name")?;

    let category = opts.category.clone().unwrap_or_else(|| infer_category(src));

    let dest_dir = paths.library.join(&category);
    std::fs::create_dir_all(&dest_dir)
        .with_context(|| format!("failed to create {}", dest_dir.display()))?;

    match resolve_dest(&dest_dir, file_name, &sha)? {
        Dest::Dedup(path) => {
            let rel = rel_path(paths, &path)?;
            Ok(Some(Added {
                abs_path: path,
                rel_path: rel,
                deduped: true,
            }))
        }
        Dest::New(dest) => {
            std::fs::write(&dest, &bytes)
                .with_context(|| format!("failed to write {}", dest.display()))?;
            let now = OffsetDateTime::now_utc();
            let sidecar = Sidecar {
                id: Ulid::new().to_string(),
                tags: opts.tags.clone(),
                description: opts.description.clone().unwrap_or_default(),
                language: None,
                source: Some(src.display().to_string()),
                added: now,
                updated: now,
                uses: 0,
                sha256: sha,
            };
            sidecar.save(&dest)?;
            let rel = rel_path(paths, &dest)?;
            Ok(Some(Added {
                abs_path: dest,
                rel_path: rel,
                deduped: false,
            }))
        }
    }
}

/// a chosen destination, distinguishing a fresh copy from a content-identical dup.
enum Dest {
    Dedup(PathBuf),
    New(PathBuf),
}

/// picks a destination path inside `dest_dir`, suffixing on name collisions and
/// short-circuiting when an identical file (same sha256) is already present.
fn resolve_dest(dest_dir: &Path, file_name: &str, sha: &str) -> Result<Dest> {
    let (stem, ext) = split_name(file_name);
    let mut candidate = dest_dir.join(file_name);
    let mut n = 0u32;
    loop {
        if !candidate.exists() {
            return Ok(Dest::New(candidate));
        }
        let existing = std::fs::read(&candidate)
            .with_context(|| format!("failed to read existing {}", candidate.display()))?;
        if sha256_hex(&existing) == sha {
            return Ok(Dest::Dedup(candidate));
        }
        n += 1;
        let next = match &ext {
            Some(e) => format!("{stem}-{n}.{e}"),
            None => format!("{stem}-{n}"),
        };
        candidate = dest_dir.join(next);
    }
}

/// returns the library-relative path with forward slashes.
fn rel_path(paths: &Paths, full: &Path) -> Result<String> {
    let rel = full
        .strip_prefix(&paths.library)
        .with_context(|| format!("{} is outside the library", full.display()))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

/// splits a file name into (stem, extension), treating dotfiles as extension-less.
fn split_name(file_name: &str) -> (String, Option<String>) {
    match file_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), Some(ext.to_string())),
        _ => (file_name.to_string(), None),
    }
}

/// maps a file to a category directory by name/extension; falls back to `misc`.
fn infer_category(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match name {
        "Dockerfile" | "Containerfile" => return "docker".to_string(),
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
            return "docker".to_string();
        }
        "Makefile" | "makefile" | "justfile" | "Justfile" => return "make".to_string(),
        _ => {}
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let category = match ext {
        "sh" | "bash" | "zsh" => "bash",
        "ps1" | "psm1" => "powershell",
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "vue" => "vue",
        "svelte" => "svelte",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "cs" => "csharp",
        "fs" | "fsx" => "fsharp",
        "swift" => "swift",
        "php" => "php",
        "scala" | "sc" => "scala",
        "dart" => "dart",
        "zig" => "zig",
        "nim" => "nim",
        "cr" => "crystal",
        "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "hs" | "lhs" => "haskell",
        "ml" | "mli" => "ocaml",
        "clj" | "cljs" | "cljc" => "clojure",
        "jl" => "julia",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "r" => "r",
        "sql" => "sql",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "json" => "json",
        "tf" | "tfvars" | "hcl" => "terraform",
        "nix" => "nix",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "vim" => "vim",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "md" | "markdown" => "markdown",
        _ => "misc",
    };
    category.to_string()
}

/// reads the head of a file and reports whether it looks binary.
pub fn is_binary(path: &Path) -> Result<bool> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf)?;
    Ok(is_binary_bytes(&buf[..n]))
}

/// a NUL byte in the head is the classic "binary" signal, same heuristic git uses.
fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

/// hex-encoded sha256 of the given bytes.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// removes an entry's content file and its sidecar, pruning an emptied category.
pub fn remove(paths: &Paths, rel_path: &str) -> Result<()> {
    let content = paths.library.join(rel_path);
    if !content.exists() {
        bail!("no such entry: {rel_path}");
    }
    std::fs::remove_file(&content)
        .with_context(|| format!("failed to remove {}", content.display()))?;
    let sidecar = Sidecar::path_for(&content);
    if sidecar.exists() {
        let _ = std::fs::remove_file(&sidecar);
    }
    prune_empty_parent(&content);
    Ok(())
}

/// moves/renames an entry (and its sidecar) to a new library-relative path.
pub fn rename(paths: &Paths, from_rel: &str, to_rel: &str) -> Result<()> {
    let from = paths.library.join(from_rel);
    if !from.exists() {
        bail!("no such entry: {from_rel}");
    }
    let to = paths.library.join(to_rel);
    if to.exists() {
        bail!("destination already exists: {to_rel}");
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::rename(&from, &to)
        .with_context(|| format!("failed to move {} -> {}", from.display(), to.display()))?;
    let from_sidecar = Sidecar::path_for(&from);
    if from_sidecar.exists() {
        let _ = std::fs::rename(&from_sidecar, Sidecar::path_for(&to));
    }
    prune_empty_parent(&from);
    Ok(())
}

/// applies tag edits (set, then add, then remove) to an entry's sidecar.
pub fn edit_tags(
    paths: &Paths,
    rel_path: &str,
    add: &[String],
    remove: &[String],
    set: Option<Vec<String>>,
) -> Result<Vec<String>> {
    let mut sidecar = load_sidecar(paths, rel_path)?;
    if let Some(tags) = set {
        sidecar.tags = tags;
    }
    for tag in add {
        if !sidecar.tags.contains(tag) {
            sidecar.tags.push(tag.clone());
        }
    }
    sidecar.tags.retain(|tag| !remove.contains(tag));
    sidecar.updated = OffsetDateTime::now_utc();
    sidecar.save(&paths.library.join(rel_path))?;
    Ok(sidecar.tags.clone())
}

/// sets an entry's description in its sidecar.
pub fn set_description(paths: &Paths, rel_path: &str, description: &str) -> Result<()> {
    let mut sidecar = load_sidecar(paths, rel_path)?;
    sidecar.description = description.to_string();
    sidecar.updated = OffsetDateTime::now_utc();
    sidecar.save(&paths.library.join(rel_path))?;
    Ok(())
}

fn load_sidecar(paths: &Paths, rel_path: &str) -> Result<Sidecar> {
    let content = paths.library.join(rel_path);
    let sidecar_path = Sidecar::path_for(&content);
    if !sidecar_path.exists() {
        bail!("no sidecar for {rel_path}; run `chimera reindex` first");
    }
    Sidecar::load(&sidecar_path)
}

/// removes the entry's parent directory if it is now empty (best effort).
fn prune_empty_parent(content: &Path) {
    if let Some(parent) = content.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

/// recomputes a stored entry's content hash and bumps `updated`, after an edit.
pub(crate) fn rehash(paths: &Paths, rel_path: &str) -> Result<()> {
    let content = paths.library.join(rel_path);
    let sidecar_path = Sidecar::path_for(&content);
    if !sidecar_path.exists() {
        return Ok(());
    }
    let bytes =
        std::fs::read(&content).with_context(|| format!("failed to read {}", content.display()))?;
    let mut sidecar = Sidecar::load(&sidecar_path)?;
    sidecar.sha256 = sha256_hex(&bytes);
    sidecar.updated = OffsetDateTime::now_utc();
    sidecar.save(&content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_name_handles_dotfiles_and_extensions() {
        assert_eq!(
            split_name("deploy.sh"),
            ("deploy".into(), Some("sh".into()))
        );
        assert_eq!(split_name("a.b.c"), ("a.b".into(), Some("c".into())));
        assert_eq!(split_name(".bashrc"), (".bashrc".into(), None));
        assert_eq!(split_name("Makefile"), ("Makefile".into(), None));
    }

    #[test]
    fn infer_category_by_name_and_extension() {
        assert_eq!(infer_category(Path::new("x.rs")), "rust");
        assert_eq!(infer_category(Path::new("x.zig")), "zig");
        assert_eq!(infer_category(Path::new("Dockerfile")), "docker");
        assert_eq!(infer_category(Path::new("docker-compose.yml")), "docker");
        assert_eq!(infer_category(Path::new("weird.qzx")), "misc");
    }

    #[test]
    fn binary_detection_uses_nul_byte() {
        assert!(is_binary_bytes(b"abc\0def"));
        assert!(!is_binary_bytes(b"plain text"));
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        // sha256("abc")
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
