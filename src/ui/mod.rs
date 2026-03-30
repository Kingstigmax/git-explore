pub mod blame;
pub mod diff_panel;
pub mod filetree;
pub mod help;
pub mod highlight;
pub mod search;
pub mod timeline;

use ratatui::style::{Color, Style};

/// App-wide color palette (dark theme)
pub struct Theme;

impl Theme {
    pub const BG: Color = Color::Rgb(18, 18, 24);
    pub const SURFACE: Color = Color::Rgb(28, 28, 36);
    pub const BORDER: Color = Color::Rgb(60, 60, 80);
    pub const TEXT: Color = Color::Rgb(210, 210, 220);
    pub const DIM: Color = Color::Rgb(100, 100, 120);
    pub const ACCENT: Color = Color::Rgb(120, 180, 255);
    pub const GREEN: Color = Color::Rgb(80, 200, 120);
    pub const RED: Color = Color::Rgb(220, 80, 80);
    pub const YELLOW: Color = Color::Rgb(240, 200, 80);
    pub const ORANGE: Color = Color::Rgb(240, 140, 60);
    pub const PURPLE: Color = Color::Rgb(180, 120, 240);
    pub const CYAN: Color = Color::Rgb(80, 220, 200);

    pub const TAB_ACTIVE: Color = Color::Rgb(120, 180, 255);
    pub const TAB_INACTIVE: Color = Color::Rgb(80, 80, 100);
    pub const SELECTED_BG: Color = Color::Rgb(40, 50, 70);

    pub fn style() -> Style {
        Style::default().fg(Self::TEXT).bg(Self::BG)
    }

    pub fn dim() -> Style {
        Style::default().fg(Self::DIM)
    }

    pub fn accent() -> Style {
        Style::default().fg(Self::ACCENT)
    }

    pub fn selected() -> Style {
        Style::default().fg(Self::TEXT).bg(Self::SELECTED_BG)
    }

    /// Age-based color: 0.0 = newest (warm), 1.0 = oldest (cool)
    pub fn age_color(age_ratio: f32) -> Color {
        let r = (220.0 * (1.0 - age_ratio) + 60.0 * age_ratio) as u8;
        let g = (120.0 * (1.0 - age_ratio) + 100.0 * age_ratio) as u8;
        let b = (60.0 * (1.0 - age_ratio) + 200.0 * age_ratio) as u8;
        Color::Rgb(r, g, b)
    }

    /// Deterministic author color from author name hash
    pub fn author_color(author: &str) -> Color {
        let hash: u64 = author
            .bytes()
            .fold(5381u64, |acc, b| acc.wrapping_mul(33).wrapping_add(b as u64));
        let palette = [
            Color::Rgb(120, 200, 140),
            Color::Rgb(140, 160, 240),
            Color::Rgb(240, 160, 100),
            Color::Rgb(200, 120, 200),
            Color::Rgb(100, 210, 210),
            Color::Rgb(230, 190, 80),
            Color::Rgb(180, 100, 120),
            Color::Rgb(100, 180, 160),
        ];
        palette[(hash % palette.len() as u64) as usize]
    }
}

/// Format a DateTime as a relative "X ago" string
pub fn relative_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let secs = (now - *dt).num_seconds().max(0) as u64;
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 86400 * 30 {
        format!("{}d ago", secs / 86400)
    } else if secs < 86400 * 365 {
        format!("{}mo ago", secs / (86400 * 30))
    } else {
        format!("{}y ago", secs / (86400 * 365))
    }
}

/// Truncate a string to max_width, appending "…" if truncated
pub fn truncate(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        s.to_string()
    } else if max_width > 1 {
        let truncated: String = chars[..max_width - 1].iter().collect();
        format!("{}…", truncated)
    } else {
        chars[..max_width].iter().collect()
    }
}
