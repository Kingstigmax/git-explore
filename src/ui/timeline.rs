use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::git::CommitInfo;
use crate::ui::{relative_time, truncate, Theme};

pub struct TimelineState {
    pub list_state: ListState,
    pub search_input: String,
    pub search_active: bool,
    pub filter_author: Option<String>,
    pub show_diff: bool,
}

impl TimelineState {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            search_input: String::new(),
            search_active: false,
            filter_author: None,
            show_diff: false,
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn select(&mut self, idx: usize, len: usize) {
        if len == 0 {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(idx.min(len.saturating_sub(1))));
        }
    }

    pub fn move_up(&mut self, commits: &[CommitInfo]) {
        let len = commits.len();
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0);
        let next = if cur == 0 { 0 } else { cur - 1 };
        self.list_state.select(Some(next));
    }

    pub fn move_down(&mut self, commits: &[CommitInfo]) {
        let len = commits.len();
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0);
        let next = (cur + 1).min(len - 1);
        self.list_state.select(Some(next));
    }
}

pub fn render_timeline(
    f: &mut Frame,
    area: Rect,
    commits: &[CommitInfo],
    state: &mut TimelineState,
    diff_content: Option<&str>,
) {
    if state.show_diff && diff_content.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        render_commit_list(f, chunks[0], commits, state);
        render_diff_panel(f, chunks[1], diff_content.unwrap_or(""));
    } else {
        render_commit_list(f, area, commits, state);
    }
}

fn render_commit_list(
    f: &mut Frame,
    area: Rect,
    commits: &[CommitInfo],
    state: &mut TimelineState,
) {
    let items: Vec<ListItem> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let selected = state.list_state.selected() == Some(i);
            build_commit_item(c, selected, area.width)
        })
        .collect();

    // Search bar at the bottom of the block title area
    let title = if state.search_active {
        format!(" Timeline  /{}█ ", state.search_input)
    } else if let Some(ref author) = state.filter_author {
        format!(" Timeline  [author: {}] ", author)
    } else {
        format!(" Timeline  ({} commits) ", commits.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Theme::SELECTED_BG)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn build_commit_item(c: &CommitInfo, _selected: bool, width: u16) -> ListItem<'static> {
    let avail = width.saturating_sub(2) as usize;

    // Hash (8 chars)
    let hash_span = Span::styled(
        c.short_hash.clone(),
        Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
    );

    // Refs (branch/tag labels)
    let mut ref_spans: Vec<Span> = Vec::new();
    for r in &c.refs {
        if r.starts_with("tag:") {
            ref_spans.push(Span::styled(
                format!(" [{}]", &r[4..]),
                Style::default().fg(Theme::YELLOW),
            ));
        } else if r.starts_with("remote/") {
            ref_spans.push(Span::styled(
                format!(" ({})", r),
                Style::default().fg(Theme::RED),
            ));
        } else {
            ref_spans.push(Span::styled(
                format!(" ({})", r),
                Style::default().fg(Theme::GREEN),
            ));
        }
    }

    // Author (up to 16 chars, colored by name)
    let author_str = truncate(&c.author, 16);
    let author_span = Span::styled(
        format!(" {:16}", author_str),
        Style::default().fg(Theme::author_color(&c.author)),
    );

    // Date (8 chars)
    let date_str = relative_time(&c.time);
    let date_span = Span::styled(
        format!(" {:>8} ", date_str),
        Style::default().fg(Theme::DIM),
    );

    // Summary (rest of line)
    let used = 8 + 1 + 16 + 1 + 8 + 1;
    let summary_width = avail.saturating_sub(used);
    let summary_str = truncate(&c.summary, summary_width);
    let summary_span = Span::styled(summary_str, Style::default().fg(Theme::TEXT));

    let mut spans = vec![hash_span];
    spans.extend(ref_spans);
    spans.push(author_span);
    spans.push(date_span);
    spans.push(summary_span);

    ListItem::new(Line::from(spans))
}

fn render_diff_panel(f: &mut Frame, area: Rect, diff_text: &str) {
    let block = Block::default()
        .title(" Diff ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let para = Paragraph::new(diff_text.to_string())
        .block(block)
        .style(Style::default().fg(Theme::TEXT))
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}
