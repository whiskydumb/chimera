use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, Focus, Mode, RowKind};

// the status bar uses the terminal's ANSI palette so it matches the user's
// terminal theme (dark or light) instead of hardcoded colors.
const BAR_BG: Color = Color::Reset;
const DARK: Color = Color::Black;
const INSERT_BG: Color = Color::Green;
const NORMAL_BG: Color = Color::Blue;
const CONFIRM_BG: Color = Color::Red;
const KEYS_BG: Color = Color::DarkGray;
const KEYS_FG: Color = Color::Gray;
const POS_BG: Color = Color::Magenta;
const SECTION_FG: Color = Color::Cyan;
const FOCUS_BORDER: Color = Color::Cyan;

// @note: powerline separators — require a Nerd Font in the terminal running
// chimera. swap these for "" (or "|") if glyphs render as boxes.
const SEP_RIGHT: &str = "\u{e0b0}"; //
const SEP_LEFT: &str = "\u{e0b2}"; //

const SEARCH_HINT: &str = " Search   'exact  ^prefix  suffix$  !not  *glob ";

pub fn render(frame: &mut Frame, app: &mut App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search bar
            Constraint::Min(1),    // body
            Constraint::Length(1), // transient message
            Constraint::Length(1), // statusline
        ])
        .split(frame.area());

    let input = Paragraph::new(app.query.as_str())
        .block(Block::default().borders(Borders::ALL).title(SEARCH_HINT));
    frame.render_widget(input, rows[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[1]);

    // borders take one row top and bottom.
    app.set_preview_viewport(body[1].height.saturating_sub(2));

    let items: Vec<ListItem> = app
        .rows
        .iter()
        .map(|row| match row {
            RowKind::Header(title) => ListItem::new(Line::from(Span::styled(
                title.clone(),
                Style::default().fg(SECTION_FG).add_modifier(Modifier::BOLD),
            ))),
            RowKind::Hit(hit) => ListItem::new(hit.record.rel_path.clone()),
        })
        .collect();
    let list_title = format!(" Results ({}) ", app.hit_count());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .border_style(border_style(app.focus == Focus::Results)),
        )
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, body[0], &mut app.list_state);

    let preview_title = app
        .selected_hit()
        .map(|hit| format!(" {} ", hit.record.rel_path))
        .unwrap_or_else(|| " Preview ".to_string());
    let preview = Paragraph::new(app.preview.clone())
        .scroll((app.preview_scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(preview_title)
                .border_style(border_style(app.focus == Focus::Preview)),
        );
    frame.render_widget(preview, body[1]);

    if !app.status.is_empty() {
        let message = Paragraph::new(app.status.as_str()).style(Style::default().fg(Color::Yellow));
        frame.render_widget(message, rows[2]);
    }

    let bar = statusline(app, rows[3].width);
    frame.render_widget(Paragraph::new(bar).style(Style::default().bg(BAR_BG)), rows[3]);
}

/// accents the border of the focused pane.
fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(FOCUS_BORDER)
    } else {
        Style::default()
    }
}

/// builds the lualine-style status bar: a vim-mode block and the current mode's
/// keybinds on the left, the selection position on the right.
fn statusline(app: &App, width: u16) -> Line<'static> {
    let mode_bg = if app.is_confirming() {
        CONFIRM_BG
    } else {
        match app.vim_mode() {
            Mode::Insert => INSERT_BG,
            Mode::Normal => NORMAL_BG,
        }
    };
    let left: Vec<Span<'static>> = vec![
        Span::styled(
            format!(" {} ", app.mode_label()),
            Style::default().fg(DARK).bg(mode_bg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(SEP_RIGHT, Style::default().fg(mode_bg).bg(KEYS_BG)),
        Span::styled(format!(" {} ", app.binds()), Style::default().fg(KEYS_FG).bg(KEYS_BG)),
        Span::styled(SEP_RIGHT, Style::default().fg(KEYS_BG).bg(BAR_BG)),
    ];

    let position = format!(" {}/{} ", app.current_pos(), app.hit_count());
    let right: Vec<Span<'static>> = vec![
        Span::styled(SEP_LEFT, Style::default().fg(POS_BG).bg(BAR_BG)),
        Span::styled(
            position,
            Style::default().fg(DARK).bg(POS_BG).add_modifier(Modifier::BOLD),
        ),
    ];

    let used: usize = left
        .iter()
        .chain(right.iter())
        .map(|span| span.content.chars().count())
        .sum();
    let filler = (width as usize).saturating_sub(used);

    let mut spans = left;
    spans.push(Span::styled(" ".repeat(filler), Style::default().bg(BAR_BG)));
    spans.extend(right);
    Line::from(spans)
}
