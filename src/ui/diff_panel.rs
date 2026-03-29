use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::git::diff::{DiffResult, FileDiff};
use crate::ui::Theme;

pub struct DiffPanelState {
    pub list_state: ListState,
    pub flat_lines: Vec<FlatLine>,
    pub scroll: u16,
}

#[derive(Clone)]
pub enum FlatLine {
    FileHeader(String),
    HunkHeader(String),
    Added(String),
    Removed(String),
    Context(String),
    Stats(String),
}

impl DiffPanelState {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default(),
            flat_lines: Vec::new(),
            scroll: 0,
        }
    }

    pub fn load(&mut self, diff: &DiffResult) {
        self.flat_lines = flatten_diff(diff);
        self.list_state.select(Some(0));
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self
            .scroll
            .saturating_add(3)
            .min(self.flat_lines.len().saturating_sub(1) as u16);
    }
}

fn flatten_diff(diff: &DiffResult) -> Vec<FlatLine> {
    let mut lines = Vec::new();
    lines.push(FlatLine::Stats(diff.stats_text.clone()));
    for file in &diff.files {
        lines.push(FlatLine::FileHeader(file_header(file)));
        for hunk in &file.hunks {
            lines.push(FlatLine::HunkHeader(hunk.header.clone()));
            for line in &hunk.lines {
                match line.origin {
                    '+' => lines.push(FlatLine::Added(line.content.clone())),
                    '-' => lines.push(FlatLine::Removed(line.content.clone())),
                    _ => lines.push(FlatLine::Context(line.content.clone())),
                }
            }
        }
    }
    lines
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

pub fn render_diff_panel(
    f: &mut Frame,
    area: Rect,
    state: &mut DiffPanelState,
    title: &str,
) {
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let items: Vec<ListItem> = state
        .flat_lines
        .iter()
        .skip(state.scroll as usize)
        .map(|fl| flat_line_to_item(fl))
        .collect();

    let list = List::new(items).block(block);
    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn flat_line_to_item(fl: &FlatLine) -> ListItem<'static> {
    match fl {
        FlatLine::FileHeader(s) => ListItem::new(Line::from(Span::styled(
            s.clone(),
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ))),
        FlatLine::HunkHeader(s) => ListItem::new(Line::from(Span::styled(
            s.clone(),
            Style::default().fg(Theme::CYAN),
        ))),
        FlatLine::Added(s) => ListItem::new(Line::from(vec![
            Span::styled("+", Style::default().fg(Theme::GREEN)),
            Span::styled(s.clone(), Style::default().fg(Theme::GREEN)),
        ])),
        FlatLine::Removed(s) => ListItem::new(Line::from(vec![
            Span::styled("-", Style::default().fg(Theme::RED)),
            Span::styled(s.clone(), Style::default().fg(Theme::RED)),
        ])),
        FlatLine::Context(s) => ListItem::new(Line::from(Span::styled(
            format!(" {}", s),
            Style::default().fg(Theme::TEXT),
        ))),
        FlatLine::Stats(s) => ListItem::new(Line::from(Span::styled(
            s.clone(),
            Style::default().fg(Theme::DIM),
        ))),
    }
}
