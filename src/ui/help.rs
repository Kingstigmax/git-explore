use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::Theme;

pub fn render_help_overlay(f: &mut Frame, area: Rect) {
    // Center a modal
    let modal_w = 62u16.min(area.width.saturating_sub(4));
    let modal_h = 32u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_h)) / 2;
    let modal_area = Rect::new(x, y, modal_w, modal_h);

    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Help — git-explorer ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::ACCENT))
        .style(Style::default().bg(Theme::SURFACE));

    let inner = block.inner(modal_area);
    f.render_widget(block, modal_area);

    let sections = help_text();
    let lines: Vec<Line> = sections
        .iter()
        .flat_map(|(heading, binds)| {
            let mut out = vec![
                Line::raw(""),
                Line::from(Span::styled(
                    *heading,
                    Style::default()
                        .fg(Theme::ACCENT)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )),
            ];
            for (key, desc) in binds.iter() {
                out.push(Line::from(vec![
                    Span::styled(
                        format!("  {:14}", key),
                        Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(desc.to_string(), Style::default().fg(Theme::TEXT)),
                ]));
            }
            out
        })
        .collect();

    let para = Paragraph::new(lines).style(Style::default().bg(Theme::SURFACE));
    f.render_widget(para, inner);
}

fn help_text() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "Navigation",
            vec![
                ("j / ↓", "Move down"),
                ("k / ↑", "Move up"),
                ("g / Home", "Go to top"),
                ("G / End", "Go to bottom"),
                ("Ctrl+d", "Page down"),
                ("Ctrl+u", "Page up"),
            ],
        ),
        (
            "Views (Tab or number)",
            vec![
                ("1", "Timeline — commit history"),
                ("2", "File Tree — files at commit"),
                ("3", "Search — search history"),
                ("4", "Blame — line-by-line blame"),
                ("Tab", "Cycle to next view"),
            ],
        ),
        (
            "Timeline",
            vec![
                ("Enter", "Toggle diff panel"),
                ("d", "Diff vs HEAD"),
                ("/", "Search commits"),
                ("a", "Filter by author"),
                ("y", "Copy commit hash"),
            ],
        ),
        (
            "File Tree",
            vec![
                ("Enter", "Expand dir / preview file"),
                ("b", "Blame selected file"),
                ("h", "File history"),
                ("o", "Open in $EDITOR"),
            ],
        ),
        (
            "Blame",
            vec![
                ("Enter", "Jump to commit in Timeline"),
                ("p", "Parent blame (before this commit)"),
                ("y", "Copy commit hash"),
                ("o", "Open file in $EDITOR at line"),
            ],
        ),
        (
            "Search",
            vec![
                ("Tab", "Cycle search mode"),
                ("Enter", "Run search / jump to commit"),
                ("Esc", "Cancel input"),
            ],
        ),
        (
            "Global",
            vec![
                ("?", "Toggle this help"),
                ("q / Esc", "Quit / back"),
                ("y", "Yank (copy) to clipboard"),
            ],
        ),
    ]
}
