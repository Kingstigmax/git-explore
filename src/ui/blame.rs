use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::git::BlameResult;
use crate::ui::{relative_time, Theme};

pub struct BlameViewState {
    pub list_state: ListState,
    pub file_path: String,
}

impl BlameViewState {
    pub fn new(file_path: String) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { list_state, file_path }
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn move_up(&mut self, len: usize) {
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if cur == 0 { 0 } else { cur - 1 }));
    }

    pub fn move_down(&mut self, len: usize) {
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((cur + 1).min(len - 1)));
    }

    pub fn page_up(&mut self, page_size: usize) {
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(cur.saturating_sub(page_size)));
    }

    pub fn page_down(&mut self, total: usize, page_size: usize) {
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((cur + page_size).min(total.saturating_sub(1))));
    }
}

pub fn render_blame(
    f: &mut Frame,
    area: Rect,
    blame: Option<&BlameResult>,
    state: &mut BlameViewState,
) {
    match blame {
        None => {
            let block = Block::default()
                .title(format!(" Blame: {} ", state.file_path))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Theme::BORDER))
                .style(Style::default().bg(Theme::BG));
            let loading = ratatui::widgets::Paragraph::new("Loading blame…")
                .block(block)
                .style(Style::default().fg(Theme::DIM));
            f.render_widget(loading, area);
        }
        Some(blame) => render_blame_content(f, area, blame, state),
    }
}

fn render_blame_content(
    f: &mut Frame,
    area: Rect,
    blame: &BlameResult,
    state: &mut BlameViewState,
) {
    let now = chrono::Utc::now();
    let oldest = blame.oldest_time;
    let newest = blame.newest_time;
    let time_span = (newest - oldest).num_seconds().max(1) as f32;

    // Determine widths
    let lineno_w = digits(blame.lines.len()) + 1;
    let hash_w = 8usize;
    let author_w = 12usize;
    let date_w = 8usize;
    let meta_w = lineno_w + hash_w + author_w + date_w + 4; // spaces
    let code_w = (area.width as usize).saturating_sub(meta_w + 3);

    let items: Vec<ListItem> = blame
        .lines
        .iter()
        .map(|bl| {
            let age_ratio = if time_span > 0.0 {
                1.0 - ((bl.time - oldest).num_seconds() as f32 / time_span)
            } else {
                0.0
            };
            let age_color = Theme::age_color(age_ratio.clamp(0.0, 1.0));
            let author_color = Theme::author_color(&bl.author);

            let lineno_str = format!("{:>width$} ", bl.lineno, width = lineno_w);
            let hash_str = format!("{:8} ", bl.short_hash);
            let author_str = format!("{:<12} ", truncate_str(&bl.author, 12));
            let date_str = format!("{:>8} ", relative_time(&bl.time));
            let code_str = truncate_str(&bl.content, code_w);

            ListItem::new(Line::from(vec![
                Span::styled(lineno_str, Style::default().fg(Theme::DIM)),
                Span::styled(
                    hash_str,
                    Style::default().fg(age_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(author_str, Style::default().fg(author_color)),
                Span::styled(date_str, Style::default().fg(Theme::DIM)),
                Span::styled(code_str, Style::default().fg(Theme::TEXT)),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" Blame: {} ", blame.file_path))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Theme::SELECTED_BG).add_modifier(Modifier::BOLD));

    f.render_stateful_widget(list, area, &mut state.list_state);
    let _ = now;
}

fn digits(n: usize) -> usize {
    if n == 0 { 1 } else { (n as f64).log10() as usize + 1 }
}

fn truncate_str(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else if max > 1 {
        format!("{}…", chars[..max - 1].iter().collect::<String>())
    } else {
        chars[..max].iter().collect()
    }
}
