mod app;
mod highlight;
mod ui;

use std::io::IsTerminal;

use anyhow::{bail, Context, Result};
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

use crate::config::{Config, Paths};
use crate::index::db;
use crate::tui::app::App;

/// launches the interactive TUI.
pub fn run(paths: Paths) -> Result<()> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        bail!("the chimera TUI needs an interactive terminal; use `chimera search <query>` in pipelines");
    }

    let cwd = std::env::current_dir().context("cannot determine the current directory")?;
    let config = Config::load(&paths)?;
    let conn = db::open(&paths)?;
    let mut app = App::new(paths, config, cwd, conn);
    app.refresh_results()?;

    let mut terminal = ratatui::init();
    enable_mouse();
    let outcome = app.run_loop(&mut terminal);
    disable_mouse();
    ratatui::restore();

    if let Some(path) = outcome.context("the TUI event loop failed")? {
        println!("{}", path.display());
    }
    Ok(())
}

/// enables mouse capture so the wheel yields scroll events we step through one
/// at a time, instead of the terminal translating it into a burst of arrow keys.
fn enable_mouse() {
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
}

fn disable_mouse() {
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
}
