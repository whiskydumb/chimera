<div align="center">

# chimera

**personal arsenal — store, search and reuse your code, scripts and templates from one fast TUI.**

[![ci](https://github.com/whiskydumb/chimera/actions/workflows/ci.yml/badge.svg)](https://github.com/whiskydumb/chimera/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-Zlib-blue?style=flat-square)](LICENSE)

<img src="https://raw.githubusercontent.com/whiskydumb/chimera/main/.github/assets/demo.png" alt="chimera demo" width="720" />

</div>

Over time a developer accumulates hundreds of useful snippets — bash scripts, Docker Compose files,
GitHub Actions, SQL queries, Vim configs — scattered across old repos and machines. Finding one
becomes harder than rewriting it. **chimera** turns that pile into a single local library with
instant search and a keyboard-driven terminal UI.

Files stay plain files on disk. SQLite (FTS5) is only an index.

## Features

- **Store** any text artifact — add a single file or a whole directory (`chimera add ./templates/`).
- **Search** by name, path, extension, tags, or full file content — fast even with thousands of entries.
- **Sectioned results** — fuzzy matches on *names* and full-text matches in *content*, shown separately.
- **Operators** (fzf-style): `'exact`, `^prefix`, `suffix$`, `!not`, `*.glob`, space = AND.
- **TUI** with a live split view: results on the left, syntax-highlighted preview on the right.
- **Vim keybindings** (modal) that work on non-latin layouts via a configurable `langmap`.
- **Reuse** — copy an entry into the current project or the clipboard with one key.
- **`$EDITOR` integration**, a live filesystem **watcher**, and per-entry tags/description.
- Cross-platform: Linux, macOS, Windows.

## Installation

Not published anywhere yet — build it from source (requires Rust 1.88+):

```sh
git clone https://github.com/whiskydumb/chimera
cd chimera
cargo build --release
./target/release/chimera        # run it; or: cargo run --release
```

To put the `chimera` binary on your `PATH`:

```sh
cargo install --path .
```

Packaged installs (cargo, nix, distro packages) will land once it's released.

## Usage

### CLI

```sh
chimera add ./deploy.sh --tags deploy,ssh    # add a file (or a directory) to the library
chimera add ./templates/                     # add a directory recursively
chimera search docker                        # search names + content
chimera search '^py'                         # prefix; also 'exact, suffix$, !not, *.glob
chimera list                                 # list everything
chimera copy bash/deploy.sh --to .           # copy an entry into a directory
chimera edit bash/deploy.sh                  # open it in $EDITOR
chimera mv bash/deploy.sh bash/release.sh    # rename / move
chimera tag bash/deploy.sh --add prod        # edit tags
chimera describe bash/deploy.sh "zero-downtime deploy"
chimera rm bash/deploy.sh                    # remove
chimera init                                 # write a default config.toml
```

Running `chimera` with no arguments opens the TUI. On `Enter` it copies the selected entry into the
directory you launched from and prints the path — so it composes like fzf: `vim "$(chimera)"`.

### TUI

It opens in **INSERT** mode (just type to search). `Esc` drops to **NORMAL** mode for vim navigation.

| key | action |
|---|---|
| type / `Backspace` | edit the query (INSERT) |
| `Esc` | INSERT → NORMAL |
| `i` / `a` / `/` | NORMAL → INSERT |
| `j` / `k` (or `↑`/`↓`) | move selection (or scroll the preview when it's focused) |
| `g` / `G` | top / bottom |
| `Ctrl-d` / `Ctrl-u` | half-page |
| `Tab` (or `h` / `l`) | switch focus: results ⇄ preview |
| `Enter` | copy the entry into the launch directory, then exit |
| `y` | copy contents to the clipboard |
| `e` | open in `$EDITOR` |
| `dd` | delete the entry (asks `y`/`n`) |
| `q` / `Ctrl-c` | quit |

NORMAL-mode keys work on non-latin keyboard layouts (Russian ЙЦУКЕН by default) — see `langmap` below.

## Configuration

Settings live in `~/.chimera/config.toml` (run `chimera init` to create it):

```toml
# editor used by `chimera edit` / Ctrl-E; falls back to $VISUAL / $EDITOR / platform default.
# editor = "nvim"

# preview syntax theme: base16-ocean.dark, nord, dracula, gruvbox-dark, solarized-dark,
# catppuccin-mocha, monokai, one-half-dark, github, zenburn, ... (and more)
theme = "base16-ocean.dark"

# Nerd-Font powerline glyphs in the status bar; set false on terminals without a Nerd Font.
nerd_fonts = true

# NORMAL-mode keyboard-layout map (vim `langmap`-style); default maps Russian ЙЦУКЕН.
# langmap = "йq цw уe кr ... оj лk дl ..."
```

## How it works

```text
~/.chimera/
├── config.toml
├── index.sqlite          # FTS5 search index (a rebuildable cache)
└── library/
    ├── bash/deploy.sh
    ├── bash/deploy.sh.chm.toml   # sidecar: tags, description, hash, timestamps
    └── ...
```

Content files are kept verbatim, so they remain usable with any tool. Each entry has a small sidecar
holding its metadata, which means the SQLite index can be deleted and rebuilt at any time
(`chimera reindex`). Override the location with `CHIMERA_HOME`; control logging with `CHIMERA_LOG`
(e.g. `CHIMERA_LOG=debug`).

## Philosophy

chimera is not an IDE, a cloud service, or a package manager. It's a personal arsenal that gathers
your scattered bits of code and automation in one place and finds them in seconds.

## License

[Zlib](LICENSE).
