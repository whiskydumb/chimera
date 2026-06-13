use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use ratatui::text::Text;
use ratatui::widgets::ListState;
use ratatui::DefaultTerminal;
use rusqlite::Connection;

use crate::config::{Config, Paths};
use crate::editor;
use crate::index::db;
use crate::index::reindex;
use crate::index::search::{self, Hit, Results};
use crate::library::store;
use crate::reuse;
use crate::tui::highlight::Highlighter;
use crate::tui::ui;

const PREVIEW_MAX_LINES: usize = 250;
const RESULT_LIMIT: usize = 200;
const POLL: Duration = Duration::from_millis(200);

/// which pane currently receives navigation keys and the mouse wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Results,
    Preview,
}

/// a row in the results list: a non-selectable section header, or a hit.
pub enum RowKind {
    Header(String),
    Hit(Hit),
}

/// the whole TUI state.
pub struct App {
    pub paths: Paths,
    config: Config,
    /// directory the user launched chimera from; the target for "copy here".
    cwd: PathBuf,
    conn: Connection,
    highlighter: Highlighter,
    pub query: String,
    pub rows: Vec<RowKind>,
    /// indices into `rows` that are selectable hits (skips headers).
    hit_positions: Vec<usize>,
    /// ordinal into `hit_positions` of the current selection.
    sel: usize,
    pub list_state: ListState,
    pub preview: Text<'static>,
    pub status: String,
    pub focus: Focus,
    pub preview_scroll: u16,
    preview_lines: usize,
    preview_viewport: u16,
    outcome: Option<PathBuf>,
    pending_edit: Option<String>,
    /// rel-path of an entry awaiting delete confirmation.
    confirm: Option<String>,
    should_quit: bool,
}

impl App {
    pub fn new(paths: Paths, config: Config, cwd: PathBuf, conn: Connection) -> Self {
        let highlighter = Highlighter::new(&config.theme);
        Self {
            paths,
            config,
            cwd,
            conn,
            highlighter,
            query: String::new(),
            rows: Vec::new(),
            hit_positions: Vec::new(),
            sel: 0,
            list_state: ListState::default(),
            preview: Text::default(),
            status: String::new(),
            focus: Focus::Results,
            preview_scroll: 0,
            preview_lines: 0,
            preview_viewport: 0,
            outcome: None,
            pending_edit: None,
            confirm: None,
            should_quit: false,
        }
    }

    /// the render/input loop. returns the chosen entry's absolute path, if any.
    pub fn run_loop(
        &mut self,
        terminal: &mut DefaultTerminal,
        fs_events: &Receiver<()>,
    ) -> Result<Option<PathBuf>> {
        loop {
            terminal.draw(|frame| ui::render(frame, self))?;
            if event::poll(POLL)? {
                match event::read()? {
                    // @note: windows emits press+release; only act on press.
                    Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key)?,
                    Event::Mouse(mouse) => self.on_mouse(mouse),
                    _ => {}
                }
            }
            if let Some(rel) = self.pending_edit.take() {
                self.run_editor(terminal, &rel)?;
            }
            // coalesce a burst of filesystem events into a single reload.
            let mut changed = false;
            while fs_events.try_recv().is_ok() {
                changed = true;
            }
            if changed {
                self.reload()?;
            }
            if self.should_quit {
                return Ok(self.outcome.take());
            }
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.confirm.is_some() {
            return self.handle_confirm(key);
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c' | 'q') if ctrl => self.should_quit = true,
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Char('e') if ctrl => self.pending_edit = self.selected_rel(),
            KeyCode::Char('d') if ctrl => self.request_delete(),
            KeyCode::Char('y') if ctrl => self.copy_to_clipboard(),
            KeyCode::Char('n') if ctrl => self.select_step(1),
            KeyCode::Char('p') if ctrl => self.select_step(-1),
            KeyCode::Enter => self.reuse_into_cwd(),
            KeyCode::Up => self.nav(-1),
            KeyCode::Down => self.nav(1),
            KeyCode::PageUp => self.nav_page(-1),
            KeyCode::PageDown => self.nav_page(1),
            KeyCode::Backspace => {
                self.query.pop();
                self.focus = Focus::Results;
                self.refresh_results()?;
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.push(c);
                self.focus = Focus::Results;
                self.refresh_results()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.nav(1),
            MouseEventKind::ScrollUp => self.nav(-1),
            _ => {}
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Results => Focus::Preview,
            Focus::Preview => Focus::Results,
        };
    }

    /// up/down: move the selection in Results focus, scroll the preview otherwise.
    fn nav(&mut self, delta: i32) {
        match self.focus {
            Focus::Results => self.select_step(delta),
            Focus::Preview => self.scroll_preview(delta),
        }
    }

    fn nav_page(&mut self, dir: i32) {
        match self.focus {
            Focus::Results => self.select_page(dir * 10),
            Focus::Preview => {
                let page = self.preview_viewport.max(1) as i32;
                self.scroll_preview(dir * page);
            }
        }
    }

    /// single-step selection across sections with wrap-around (headers skipped).
    fn select_step(&mut self, delta: i32) {
        let n = self.hit_positions.len();
        if n == 0 {
            return;
        }
        self.sel = (self.sel as i32 + delta).rem_euclid(n as i32) as usize;
        self.sync_selection();
        self.refresh_preview();
    }

    /// multi-step selection jump, clamped (no wrap).
    fn select_page(&mut self, delta: i32) {
        let n = self.hit_positions.len();
        if n == 0 {
            return;
        }
        self.sel = (self.sel as i32 + delta).clamp(0, n as i32 - 1) as usize;
        self.sync_selection();
        self.refresh_preview();
    }

    fn sync_selection(&mut self) {
        self.list_state.select(self.hit_positions.get(self.sel).copied());
    }

    fn scroll_preview(&mut self, delta: i32) {
        let viewport = self.preview_viewport.max(1) as usize;
        let max = self.preview_lines.saturating_sub(viewport) as i32;
        let next = (self.preview_scroll as i32 + delta).clamp(0, max.max(0));
        self.preview_scroll = next as u16;
    }

    pub fn selected_hit(&self) -> Option<&Hit> {
        let row = *self.hit_positions.get(self.sel)?;
        match &self.rows[row] {
            RowKind::Hit(hit) => Some(hit),
            RowKind::Header(_) => None,
        }
    }

    fn selected_rel(&self) -> Option<String> {
        self.selected_hit().map(|hit| hit.record.rel_path.clone())
    }

    /// number of selectable hits across both sections.
    pub fn hit_count(&self) -> usize {
        self.hit_positions.len()
    }

    /// 1-based position of the current selection (0 when empty).
    pub fn current_pos(&self) -> usize {
        if self.hit_positions.is_empty() {
            0
        } else {
            self.sel + 1
        }
    }

    /// the statusline mode label, reflecting a pending confirm or the focused pane.
    pub fn mode(&self) -> &'static str {
        if self.confirm.is_some() {
            return "CONFIRM";
        }
        match self.focus {
            Focus::Results => "SEARCH",
            Focus::Preview => "PREVIEW",
        }
    }

    /// whether a destructive action is awaiting confirmation.
    pub fn is_confirming(&self) -> bool {
        self.confirm.is_some()
    }

    /// records the preview viewport height (called by the renderer each frame),
    /// keeping the scroll offset within bounds when the area changes.
    pub fn set_preview_viewport(&mut self, height: u16) {
        self.preview_viewport = height;
        let max = self.preview_lines.saturating_sub(height.max(1) as usize) as u16;
        self.preview_scroll = self.preview_scroll.min(max);
    }

    /// reruns the search for the current query, resetting the selection to the top.
    pub fn refresh_results(&mut self) -> Result<()> {
        self.sel = 0;
        self.rebuild_rows()
    }

    /// rebuilds the sectioned rows from the current query, clamping the selection.
    fn rebuild_rows(&mut self) -> Result<()> {
        let results = search::search(&self.conn, &self.query, RESULT_LIMIT)?;
        let (rows, hit_positions) = build_rows(results);
        self.rows = rows;
        self.hit_positions = hit_positions;
        if self.sel >= self.hit_positions.len() {
            self.sel = 0;
        }
        self.sync_selection();
        self.refresh_preview();
        Ok(())
    }

    /// re-indexes the library from disk (after an external change) and re-runs the
    /// current search, keeping the selected entry when it still exists.
    fn reload(&mut self) -> Result<()> {
        reindex::rebuild(&self.conn, &self.paths)?;
        let previous = self.selected_rel();
        self.rebuild_rows()?;
        if let Some(rel) = previous {
            self.select_rel(&rel);
        }
        self.status = "library changed — reindexed".to_string();
        Ok(())
    }

    /// selects the hit with the given relative path, if it is present.
    fn select_rel(&mut self, rel: &str) {
        let found = self.hit_positions.iter().position(|&i| {
            matches!(&self.rows[i], RowKind::Hit(hit) if hit.record.rel_path == rel)
        });
        if let Some(ordinal) = found {
            self.sel = ordinal;
            self.sync_selection();
            self.refresh_preview();
        }
    }

    /// asks to delete the selected entry; the next keypress confirms (y) or cancels.
    fn request_delete(&mut self) {
        if let Some(rel) = self.selected_rel() {
            self.status = format!("delete {rel}? (y / n)");
            self.confirm = Some(rel);
        }
    }

    /// resolves a pending delete confirmation.
    fn handle_confirm(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                if let Some(rel) = self.confirm.take() {
                    store::remove(&self.paths, &rel)?;
                    db::remove(&self.conn, &rel)?;
                    self.refresh_results()?;
                    self.status = format!("removed {rel}");
                }
            }
            _ => {
                self.confirm = None;
                self.status = "delete cancelled".to_string();
            }
        }
        Ok(())
    }

    /// suspends the TUI, runs the editor on the entry, reindexes, and resumes.
    fn run_editor(&mut self, terminal: &mut DefaultTerminal, rel: &str) -> Result<()> {
        super::disable_mouse();
        ratatui::restore();
        let result = (|| -> Result<bool> {
            let ok = editor::edit(&self.paths, &self.config, rel)?;
            if ok {
                store::rehash(&self.paths, rel)?;
                let abs = self.paths.library.join(rel);
                reindex::index_one(&self.conn, &self.paths, &abs)?;
            }
            Ok(ok)
        })();
        *terminal = ratatui::init();
        super::enable_mouse();
        terminal.clear()?;
        self.status = match result {
            Ok(true) => {
                self.refresh_preview();
                format!("edited {rel}")
            }
            Ok(false) => "editor exited without success".to_string(),
            Err(err) => format!("edit failed: {err}"),
        };
        Ok(())
    }

    /// copies the selected entry into the launch directory, then exits.
    fn reuse_into_cwd(&mut self) {
        let Some(rel) = self.selected_rel() else {
            return;
        };
        match reuse::copy_into_dir(&self.paths, &rel, &self.cwd) {
            Ok(dest) => {
                self.status = format!("copied to {}", dest.display());
                self.outcome = Some(dest);
                self.should_quit = true;
            }
            Err(err) => self.status = format!("copy failed: {err}"),
        }
    }

    fn copy_to_clipboard(&mut self) {
        let Some(rel) = self.selected_rel() else {
            return;
        };
        self.status = match reuse::copy_to_clipboard(&self.paths, &rel) {
            Ok(()) => "copied to clipboard".to_string(),
            Err(err) => format!("clipboard failed: {err}"),
        };
    }

    /// loads (a capped prefix of) the selected entry into the highlighted preview.
    fn refresh_preview(&mut self) {
        self.preview_scroll = 0;
        let rel = match self.selected_hit() {
            Some(hit) => hit.record.rel_path.clone(),
            None => {
                self.preview = Text::default();
                self.preview_lines = 0;
                return;
            }
        };
        let terms = search::literal_terms(&self.query);
        let abs = self.paths.library.join(&rel);
        self.preview = match std::fs::read(&abs) {
            Ok(bytes) => {
                let content = String::from_utf8_lossy(&bytes);
                let name = rel.rsplit('/').next().unwrap_or(&rel);
                self.highlighter.render(&content, name, PREVIEW_MAX_LINES, &terms)
            }
            Err(err) => Text::raw(format!("cannot read file: {err}")),
        };
        self.preview_lines = self.preview.lines.len();
    }
}

/// turns the two result sections into a flat row list, with headers only when
/// both sections are present, plus the positions of selectable hits.
fn build_rows(results: Results) -> (Vec<RowKind>, Vec<usize>) {
    let dual = !results.names.is_empty() && !results.content.is_empty();
    let mut rows = Vec::new();
    if !results.names.is_empty() {
        if dual {
            rows.push(RowKind::Header(format!("names ({})", results.names.len())));
        }
        rows.extend(results.names.into_iter().map(RowKind::Hit));
    }
    if !results.content.is_empty() {
        if dual {
            rows.push(RowKind::Header(format!("content ({})", results.content.len())));
        }
        rows.extend(results.content.into_iter().map(RowKind::Hit));
    }
    let hit_positions = rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| matches!(row, RowKind::Hit(_)).then_some(i))
        .collect();
    (rows, hit_positions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::store::AddOptions;

    fn temp_paths() -> Paths {
        let root = std::env::temp_dir().join(format!("chimera_test_{}", ulid::Ulid::new()));
        let paths = Paths {
            library: root.join("library"),
            db: root.join("index.sqlite"),
            config: root.join("config.toml"),
            root,
        };
        paths.ensure().unwrap();
        paths
    }

    fn flatten(text: &Text) -> String {
        text.lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect()
    }

    #[test]
    fn search_sections_preview_and_reuse() {
        let paths = temp_paths();
        let src = paths.root.join("hello.sh");
        std::fs::write(&src, "#!/bin/sh\necho hello world\n").unwrap();

        let conn = db::open(&paths).unwrap();
        for added in store::add(&paths, &src, &AddOptions::default()).unwrap() {
            reindex::index_one(&conn, &paths, &added.abs_path).unwrap();
        }

        let dest_dir = paths.root.join("project");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let mut app = App::new(paths.clone(), Config::default(), dest_dir.clone(), conn);
        app.refresh_results().unwrap();

        // empty query browses everything (single names section, no header).
        assert_eq!(app.hit_count(), 1);
        assert_eq!(app.selected_hit().unwrap().record.rel_path, "bash/hello.sh");
        assert!(flatten(&app.preview).contains("hello world"));

        // a content-only query lands in the content section and still selects.
        app.query = "world".to_string();
        app.refresh_results().unwrap();
        assert_eq!(app.hit_count(), 1);
        assert_eq!(app.selected_hit().unwrap().record.rel_path, "bash/hello.sh");

        app.toggle_focus();
        assert_eq!(app.focus, Focus::Preview);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| ui::render(frame, &mut app)).unwrap();

        app.focus = Focus::Results;
        app.reuse_into_cwd();
        assert!(dest_dir.join("hello.sh").exists());
        assert!(app.outcome.is_some());

        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
