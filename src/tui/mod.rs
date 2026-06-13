mod app;
mod highlight;
mod ui;

use std::io::IsTerminal;
use std::path::Path;
use std::sync::mpsc::{self, Sender};

use anyhow::{Context, Result, bail};
use notify::{RecursiveMode, Watcher};
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

use crate::config::{Config, Paths};
use crate::index::db;
use crate::library::entry::SIDECAR_SUFFIX;
use crate::tui::app::App;

/// launches the interactive TUI.
pub fn run(paths: Paths) -> Result<()> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        bail!(
            "the chimera TUI needs an interactive terminal; use `chimera search <query>` in pipelines"
        );
    }

    let cwd = std::env::current_dir().context("cannot determine the current directory")?;
    let config = Config::load(&paths)?;
    let conn = db::open(&paths)?;
    let watch_dir = paths.library.clone();
    let mut app = App::new(paths, config, cwd, conn);
    app.refresh_results()?;

    // watch the library so external edits / git pulls reindex live.
    let (tx, fs_events) = mpsc::channel::<()>();
    let _watcher = spawn_watcher(&watch_dir, tx);

    let mut terminal = ratatui::init();
    enable_mouse();
    let outcome = app.run_loop(&mut terminal, &fs_events);
    disable_mouse();
    ratatui::restore();

    if let Some(path) = outcome.context("the TUI event loop failed")? {
        println!("{}", path.display());
    }
    Ok(())
}

/// starts a recursive watcher on the library that signals on every content
/// change (sidecar-only changes are ignored). returns `None` if it can't start;
/// the TUI then simply runs without live reload.
fn spawn_watcher(dir: &Path, tx: Sender<()>) -> Option<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && event.paths.iter().any(|path| !is_sidecar(path))
        {
            let _ = tx.send(());
        }
    })
    .ok()?;
    watcher.watch(dir, RecursiveMode::Recursive).ok()?;
    Some(watcher)
}

fn is_sidecar(path: &Path) -> bool {
    path.to_string_lossy().ends_with(SIDECAR_SUFFIX)
}

/// enables mouse capture so the wheel yields scroll events we step through one
/// at a time, instead of the terminal translating it into a burst of arrow keys.
fn enable_mouse() {
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
}

fn disable_mouse() {
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
}
