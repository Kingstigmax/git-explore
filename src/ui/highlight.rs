use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SyntectColor, ThemeSet},
    parsing::SyntaxSet,
};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

pub fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub fn to_color(c: SyntectColor) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

/// Create a stateful highlighter for a file path, based on its extension.
pub fn highlighter_for_path(file_path: &str) -> HighlightLines<'static> {
    let ss = syntax_set();
    let ts = theme_set();
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let syntax = ss
        .find_syntax_by_extension(ext)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let theme = &ts.themes["base16-ocean.dark"];
    HighlightLines::new(syntax, theme)
}

/// Highlight a single line through an existing stateful highlighter.
/// Returns colored spans (foreground only).
pub fn highlight_line_spans(h: &mut HighlightLines<'_>, line: &str) -> Vec<Span<'static>> {
    let ss = syntax_set();
    let line_nl = format!("{}\n", line);
    match h.highlight_line(&line_nl, ss) {
        Ok(ranges) => ranges
            .iter()
            .filter_map(|(style, text)| {
                let text = text.trim_end_matches('\n');
                if text.is_empty() {
                    return None;
                }
                Some(Span::styled(
                    text.to_string(),
                    Style::default().fg(to_color(style.foreground)),
                ))
            })
            .collect(),
        Err(_) => vec![Span::raw(line.to_string())],
    }
}

/// Highlight `content` using the syntax inferred from `file_path`.
/// Returns one `Line<'static>` per line of content.
pub fn highlight_file(file_path: &str, content: &str) -> Vec<Line<'static>> {
    let ss = syntax_set();
    let ts = theme_set();

    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let syntax = ss
        .find_syntax_by_extension(ext)
        .or_else(|| ss.find_syntax_by_first_line(content.lines().next().unwrap_or("")))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);

    content
        .lines()
        .map(|line| {
            let line_nl = format!("{}\n", line);
            match h.highlight_line(&line_nl, ss) {
                Ok(ranges) => {
                    let spans: Vec<Span<'static>> = ranges
                        .iter()
                        .filter_map(|(style, text)| {
                            let text = text.trim_end_matches('\n');
                            if text.is_empty() {
                                return None;
                            }
                            Some(Span::styled(
                                text.to_string(),
                                Style::default().fg(to_color(style.foreground)),
                            ))
                        })
                        .collect();
                    Line::from(spans)
                }
                Err(_) => Line::raw(line.to_string()),
            }
        })
        .collect()
}
