use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::git::CommitInfo;
use crate::ui::{relative_time, truncate, Theme};

#[derive(Clone, PartialEq)]
pub enum SearchMode {
    Commits,  // search commit messages/authors
    Pickaxe,  // search content across history
    Files,    // find files by name across history
}

impl SearchMode {
    pub fn label(&self) -> &str {
        match self {
            SearchMode::Commits => "Commits",
            SearchMode::Pickaxe => "Content",
            SearchMode::Files => "Files",
        }
    }
}

pub struct SearchState {
    pub query: String,
    pub mode: SearchMode,
    pub results: Vec<CommitInfo>,
    pub list_state: ListState,
    pub input_active: bool,
    pub status: String,
}

impl SearchState {
    pub fn new(initial_query: Option<String>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            query: initial_query.unwrap_or_default(),
            mode: SearchMode::Commits,
            results: Vec::new(),
            list_state,
            input_active: true,
            status: String::from("Type to search, Tab to switch mode, Enter to run"),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn move_up(&mut self) {
        let len = self.results.len();
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(if cur == 0 { 0 } else { cur - 1 }));
    }

    pub fn move_down(&mut self) {
        let len = self.results.len();
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((cur + 1).min(len - 1)));
    }

    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            SearchMode::Commits => SearchMode::Pickaxe,
            SearchMode::Pickaxe => SearchMode::Files,
            SearchMode::Files => SearchMode::Commits,
        };
    }
}

pub fn render_search(f: &mut Frame, area: Rect, state: &mut SearchState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // input bar
            Constraint::Length(1), // mode selector
            Constraint::Min(0),    // results
            Constraint::Length(1), // status
        ])
        .split(area);

    render_search_input(f, chunks[0], state);
    render_mode_bar(f, chunks[1], state);
    render_results(f, chunks[2], state);
    render_status_bar(f, chunks[3], state);
}

fn render_search_input(f: &mut Frame, area: Rect, state: &SearchState) {
    let cursor = if state.input_active { "█" } else { "" };
    let text = format!("{}{}", state.query, cursor);

    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if state.input_active {
            Theme::ACCENT
        } else {
            Theme::BORDER
        }))
        .style(Style::default().bg(Theme::BG));

    let para = Paragraph::new(text).block(block).style(Style::default().fg(Theme::TEXT));
    f.render_widget(para, area);
}

fn render_mode_bar(f: &mut Frame, area: Rect, state: &SearchState) {
    let modes = [SearchMode::Commits, SearchMode::Pickaxe, SearchMode::Files];
    let spans: Vec<Span> = modes
        .iter()
        .flat_map(|m| {
            let active = *m == state.mode;
            let style = if active {
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Theme::DIM)
            };
            vec![
                Span::styled(format!(" {} ", m.label()), style),
                Span::raw(" "),
            ]
        })
        .collect();

    let line = Paragraph::new(Line::from(spans)).style(Style::default().bg(Theme::BG));
    f.render_widget(line, area);
}

fn render_results(f: &mut Frame, area: Rect, state: &mut SearchState) {
    let items: Vec<ListItem> = state
        .results
        .iter()
        .map(|c| build_result_item(c, area.width))
        .collect();

    let block = Block::default()
        .title(format!(" Results ({}) ", state.results.len()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Theme::SELECTED_BG).add_modifier(Modifier::BOLD));

    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn build_result_item(c: &CommitInfo, width: u16) -> ListItem<'static> {
    let avail = width.saturating_sub(2) as usize;
    let date = relative_time(&c.time);
    let fixed = 8 + 1 + 12 + 1 + 9;
    let summary = truncate(&c.summary, avail.saturating_sub(fixed));

    ListItem::new(Line::from(vec![
        Span::styled(
            c.short_hash.clone(),
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:12}", truncate(&c.author, 12)),
            Style::default().fg(Theme::author_color(&c.author)),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>8} ", date),
            Style::default().fg(Theme::DIM),
        ),
        Span::styled(summary, Style::default().fg(Theme::TEXT)),
    ]))
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &SearchState) {
    let para = Paragraph::new(state.status.clone())
        .style(Style::default().fg(Theme::DIM).bg(Theme::SURFACE));
    f.render_widget(para, area);
}
