use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SynStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const FALLBACK_THEME: &str = "base16-ocean.dark";
const GUTTER_FG: Color = Color::Rgb(0x6c, 0x70, 0x86);
const MATCH_FG: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
const MATCH_BG: Color = Color::Rgb(0xf9, 0xe2, 0xaf);

/// owns the syntax/theme assets and renders highlighted previews.
pub struct Highlighter {
    syntaxes: SyntaxSet,
    theme: Theme,
}

impl Highlighter {
    /// loads the extended syntect syntaxes (two-face) and a default theme.
    pub fn new(theme_name: &str) -> Self {
        let syntaxes = two_face::syntax::extra_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .get(theme_name)
            .cloned()
            .unwrap_or_else(|| themes.themes[FALLBACK_THEME].clone());
        Self { syntaxes, theme }
    }

    /// produces a highlighted `Text` for `content` (capped at `max_lines`) with a
    /// left line-number gutter; occurrences of `terms` are highlighted on top.
    pub fn render(
        &self,
        content: &str,
        file_name: &str,
        max_lines: usize,
        terms: &[String],
    ) -> Text<'static> {
        let syntax = self
            .syntaxes
            .find_syntax_by_extension(extension(file_name))
            .or_else(|| {
                self.syntaxes
                    .find_syntax_by_first_line(content.lines().next().unwrap_or(""))
            })
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());

        let raws: Vec<&str> = LinesWithEndings::from(content).take(max_lines).collect();
        let gutter_width = raws.len().max(1).to_string().len();

        let needles: Vec<Vec<char>> = terms
            .iter()
            .filter(|term| !term.is_empty())
            .map(|term| term.chars().map(|c| c.to_ascii_lowercase()).collect())
            .collect();
        let match_style = Style::default()
            .fg(MATCH_FG)
            .bg(MATCH_BG)
            .add_modifier(Modifier::BOLD);

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(raws.len());
        for (i, raw) in raws.iter().enumerate() {
            // leading space gives the numbers a little breathing room from the edge.
            let mut spans = vec![Span::styled(
                format!(" {:>gutter_width$} \u{2502} ", i + 1),
                Style::default().fg(GUTTER_FG),
            )];
            match highlighter.highlight_line(raw, &self.syntaxes) {
                Ok(ranges) => spans.extend(content_spans(&ranges, &needles, match_style)),
                Err(_) => spans.push(Span::raw(raw.trim_end_matches('\n').to_string())),
            }
            lines.push(Line::from(spans));
        }
        Text::from(lines)
    }
}

fn extension(file_name: &str) -> &str {
    file_name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

/// turns syntect ranges into ratatui spans, restyling characters that fall
/// inside a search-term occurrence (case-insensitive).
fn content_spans(
    ranges: &[(SynStyle, &str)],
    needles: &[Vec<char>],
    match_style: Style,
) -> Vec<Span<'static>> {
    let mut chars: Vec<(char, Style)> = Vec::new();
    for (syn, text) in ranges {
        let style = to_style(*syn);
        for ch in text.trim_end_matches('\n').chars() {
            chars.push((ch, style));
        }
    }

    let n = chars.len();
    let mut matched = vec![false; n];
    if !needles.is_empty() && n > 0 {
        let lower: Vec<char> = chars.iter().map(|(c, _)| c.to_ascii_lowercase()).collect();
        for start in 0..n {
            for needle in needles {
                let len = needle.len();
                if len > 0 && start + len <= n && lower[start..start + len] == needle[..] {
                    for slot in matched.iter_mut().skip(start).take(len) {
                        *slot = true;
                    }
                }
            }
        }
    }

    // coalesce consecutive characters sharing the same base style + match flag.
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        let base = chars[i].1;
        let is_match = matched[i];
        let mut j = i + 1;
        while j < n && chars[j].1 == base && matched[j] == is_match {
            j += 1;
        }
        let text: String = chars[i..j].iter().map(|(c, _)| *c).collect();
        spans.push(Span::styled(text, if is_match { match_style } else { base }));
        i = j;
    }
    spans
}

fn to_style(syn: SynStyle) -> Style {
    let fg = syn.foreground;
    let mut style = Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b));
    if syn.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if syn.font_style.contains(FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if syn.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}
