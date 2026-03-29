use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::collections::HashSet;

use crate::git::tree::{EntryKind, TreeEntry};
use crate::ui::{truncate, Theme};

pub struct FileTreeState {
    pub list_state: ListState,
    pub expanded: HashSet<String>,
    pub current_path: String,
    pub file_preview: Option<String>,
    pub preview_scroll: u16,
    pub show_preview: bool,
}

impl FileTreeState {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            expanded: HashSet::new(),
            current_path: String::new(),
            file_preview: None,
            preview_scroll: 0,
            show_preview: false,
        }
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

    pub fn toggle_expand(&mut self, path: &str) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_string());
        }
    }

    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    pub fn scroll_preview_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(3);
    }

    pub fn scroll_preview_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(3);
    }
}

/// A flat view entry for rendering (expanded from the tree)
#[derive(Clone)]
pub struct FlatEntry {
    pub entry: TreeEntry,
    pub depth: usize,
    pub children: Vec<TreeEntry>, // populated if expanded directory
}

pub fn build_flat_list(
    root: &[TreeEntry],
    expanded: &HashSet<String>,
    child_map: &std::collections::HashMap<String, Vec<TreeEntry>>,
) -> Vec<FlatEntry> {
    let mut result = Vec::new();
    build_flat_recursive(root, expanded, child_map, 0, &mut result);
    result
}

fn build_flat_recursive(
    entries: &[TreeEntry],
    expanded: &HashSet<String>,
    child_map: &std::collections::HashMap<String, Vec<TreeEntry>>,
    depth: usize,
    result: &mut Vec<FlatEntry>,
) {
    for entry in entries {
        let is_expanded = expanded.contains(&entry.path);
        result.push(FlatEntry {
            entry: entry.clone(),
            depth,
            children: if is_expanded {
                child_map
                    .get(&entry.path)
                    .cloned()
                    .unwrap_or_default()
            } else {
                vec![]
            },
        });
        if is_expanded && entry.kind == EntryKind::Directory {
            if let Some(children) = child_map.get(&entry.path) {
                build_flat_recursive(children, expanded, child_map, depth + 1, result);
            }
        }
    }
}

pub fn render_filetree(
    f: &mut Frame,
    area: Rect,
    flat: &[FlatEntry],
    state: &mut FileTreeState,
    commit_short: &str,
) {
    if state.show_preview && state.file_preview.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        render_tree_list(f, chunks[0], flat, state, commit_short);
        render_file_preview(f, chunks[1], state);
    } else {
        render_tree_list(f, area, flat, state, commit_short);
    }
}

fn render_tree_list(
    f: &mut Frame,
    area: Rect,
    flat: &[FlatEntry],
    state: &mut FileTreeState,
    commit_short: &str,
) {
    let items: Vec<ListItem> = flat
        .iter()
        .map(|fe| build_tree_item(fe, area.width))
        .collect();

    let block = Block::default()
        .title(format!(" Files @ {} ", commit_short))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Theme::SELECTED_BG).add_modifier(Modifier::BOLD));

    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn build_tree_item(fe: &FlatEntry, width: u16) -> ListItem<'static> {
    let indent = "  ".repeat(fe.depth);
    let avail = (width as usize).saturating_sub(fe.depth * 2 + 4);

    let (icon, style) = match fe.entry.kind {
        EntryKind::Directory => {
            let icon = if fe.children.is_empty() { "▶ " } else { "▼ " };
            (icon, Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD))
        }
        EntryKind::Symlink => ("→ ", Style::default().fg(Theme::CYAN)),
        EntryKind::File => {
            let icon = file_icon(&fe.entry.name);
            (icon, Style::default().fg(Theme::TEXT))
        }
    };

    let name = truncate(&fe.entry.name, avail.saturating_sub(2));
    let size_str = fe
        .entry
        .size
        .map(|s| format!(" {}", format_size(s)))
        .unwrap_or_default();

    ListItem::new(Line::from(vec![
        Span::raw(indent),
        Span::styled(icon.to_string(), style),
        Span::styled(name, style),
        Span::styled(size_str, Style::default().fg(Theme::DIM)),
    ]))
}

fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "󱘗 ",
        "py" => " ",
        "js" | "ts" | "jsx" | "tsx" => " ",
        "go" => " ",
        "c" | "cc" | "cpp" | "h" | "hpp" => " ",
        "java" => " ",
        "md" | "txt" => "󰈙 ",
        "json" | "yaml" | "yml" | "toml" => " ",
        "lock" => " ",
        "sh" | "bash" | "zsh" | "fish" => " ",
        _ => " ",
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn render_file_preview(f: &mut Frame, area: Rect, state: &FileTreeState) {
    let content = state
        .file_preview
        .as_deref()
        .unwrap_or("(no preview)");

    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .style(Style::default().bg(Theme::BG));

    let para = Paragraph::new(content.to_string())
        .block(block)
        .style(Style::default().fg(Theme::TEXT))
        .wrap(Wrap { trim: false })
        .scroll((state.preview_scroll, 0));

    f.render_widget(para, area);
}
