use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::git::diff::{DiffResult, FileDiff};
use crate::ui::highlight::{highlight_line_spans, highlighter_for_path};
use crate::ui::Theme;

pub struct DiffPanelState {
    pub rendered: Text<'static>,
    pub scroll: u16,
}

impl DiffPanelState {
    pub fn new() -> Self {
        Self {
            rendered: Text::default(),
            scroll: 0,
        }
    }

    pub fn load(&mut self, diff: &DiffResult) {
        self.rendered = build_diff_text(diff);
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self
            .scroll
            .saturating_add(3)
            .min(self.rendered.lines.len().saturating_sub(1) as u16);
    }
}

fn build_diff_text(diff: &DiffResult) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from(Span::styled(
        diff.stats_text.clone(),
        Style::default().fg(Theme::DIM),
    )));

    for file in &diff.files {
        lines.push(Line::from(Span::styled(
            file_header(file),
            Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD),
        )));

        let file_path = file.new_path.as_deref().or(file.old_path.as_deref()).unwrap_or("");
        let mut h = highlighter_for_path(file_path);

        for hunk in &file.hunks {
            lines.push(Line::from(Span::styled(
                hunk.header.clone(),
                Style::default().fg(Theme::CYAN),
            )));

            for diff_line in &hunk.lines {
                let syntax_spans = highlight_line_spans(&mut h, &diff_line.content);

                let line = match diff_line.origin {
                    '+' => {
                        let mut spans =
                            vec![Span::styled("+", Style::default().fg(Theme::GREEN))];
                        spans.extend(syntax_spans);
                        Line::from(spans)
                    }
                    '-' => {
                        let mut spans =
                            vec![Span::styled("-", Style::default().fg(Theme::RED))];
                        spans.extend(syntax_spans);
                        Line::from(spans)
                    }
                    _ => {
                        let mut spans = vec![Span::raw(" ")];
                        spans.extend(syntax_spans);
                        Line::from(spans)
                    }
                };
                lines.push(line);
            }
        }
    }

    Text::from(lines)
}

fn file_header(file: &FileDiff) -> String {
    let path = file
        .new_path
        .as_deref()
        .or(file.old_path.as_deref())
        .unwrap_or("???");
    match file.status {
        'A' => format!("+ {}", path),
        'D' => format!("- {}", path),
        'R' => format!(
            "→ {} → {}",
            file.old_path.as_deref().unwrap_or(""),
            file.new_path.as_deref().unwrap_or("")
        ),
        _ => format!("~ {}", path),
    }
}

pub fn render_diff_panel(f: &mut Frame, area: Rect, state: &mut DiffPanelState, title: &str) {
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let para = Paragraph::new(state.rendered.clone())
        .block(block)
        .scroll((state.scroll, 0));

    f.render_widget(para, area);
}
