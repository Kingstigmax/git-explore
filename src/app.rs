use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Terminal,
};
use std::collections::HashMap;
use std::io::{self, Stdout};
use std::time::Duration;

use crate::cache::AppCache;
use crate::git::{
    blame::compute_blame_at,
    diff::{compute_commit_diff, compute_range_diff},
    tree::{file_content_at, tree_at_commit, TreeEntry},
    BlameResult, CommitInfo, GitRepo,
};
use crate::ui::{
    blame::{render_blame, BlameViewState},
    diff_panel::DiffPanelState,
    filetree::{build_flat_list, render_filetree, FileTreeState},
    help::render_help_overlay,
    search::{render_search, SearchMode, SearchState},
    timeline::{render_timeline, TimelineState},
    Theme,
};

const INITIAL_LOAD: usize = 500;
const LOAD_MORE: usize = 500;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViewId {
    Timeline,
    FileTree,
    Search,
    Blame,
}

impl ViewId {
    fn index(self) -> usize {
        match self {
            ViewId::Timeline => 0,
            ViewId::FileTree => 1,
            ViewId::Search => 2,
            ViewId::Blame => 3,
        }
    }
    fn from_index(i: usize) -> Self {
        match i {
            0 => ViewId::Timeline,
            1 => ViewId::FileTree,
            2 => ViewId::Search,
            3 => ViewId::Blame,
            _ => ViewId::Timeline,
        }
    }
}

pub struct App {
    repo: GitRepo,
    active_view: ViewId,
    show_help: bool,

    // Data
    commits: Vec<CommitInfo>,
    has_more_commits: bool,

    // View states
    timeline: TimelineState,
    file_tree: FileTreeState,
    search: SearchState,
    blame: BlameViewState,

    // Side panels
    diff_panel: DiffPanelState,

    // File tree data
    root_entries: Vec<TreeEntry>,
    child_map: HashMap<String, Vec<TreeEntry>>,

    // Blame data
    blame_data: Option<BlameResult>,

    cache: AppCache,
}

impl App {
    pub fn new(
        repo_path: &str,
        initial_view: ViewId,
        blame_file: Option<String>,
        search_query: Option<String>,
    ) -> Result<Self> {
        let repo = GitRepo::open(repo_path)?;
        let commits = repo.load_commits(INITIAL_LOAD)?;
        let has_more_commits = commits.len() == INITIAL_LOAD;

        let blame_file_path = blame_file.clone().unwrap_or_default();
        let blame_view = BlameViewState::new(blame_file_path.clone());

        let search = SearchState::new(search_query);

        Ok(Self {
            repo,
            active_view: initial_view,
            show_help: false,
            commits,
            has_more_commits,
            timeline: TimelineState::new(),
            file_tree: FileTreeState::new(),
            search,
            blame: blame_view,
            diff_panel: DiffPanelState::new(),
            root_entries: Vec::new(),
            child_map: HashMap::new(),
            blame_data: None,
            cache: AppCache::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(&mut stdout);
        let mut terminal = Terminal::new(backend)?;

        // Initial loads
        self.load_initial_data();

        let result = self.event_loop(&mut terminal).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn load_initial_data(&mut self) {
        // Load root file tree at HEAD (if any commits)
        if let Some(c) = self.commits.first() {
            let oid = c.oid.clone();
            if let Ok(entries) = tree_at_commit(&self.repo.repo, &oid, "") {
                self.root_entries = entries;
            }
        }

        // If initial view is blame, load blame
        if self.active_view == ViewId::Blame && !self.blame.file_path.is_empty() {
            self.load_blame(None);
        }

        // If initial view is search with a query, run it
        if self.active_view == ViewId::Search && !self.search.query.is_empty() {
            self.run_search();
        }
    }

    fn load_blame(&mut self, commit_oid: Option<&str>) {
        let file_path = self.blame.file_path.clone();
        if file_path.is_empty() {
            return;
        }
        let cache_key = format!("{}@{}", file_path, commit_oid.unwrap_or("HEAD"));

        let result = if let Some(cached) = self.cache.blames.get(&cache_key) {
            cached.clone()
        } else {
            match compute_blame_at(&self.repo.repo, &file_path, commit_oid) {
                Ok(result) => {
                    self.cache.blames.put(cache_key, result.clone());
                    result
                }
                Err(e) => {
                    eprintln!("Blame error: {}", e);
                    return;
                }
            }
        };

        self.blame.set_highlights(&result.lines);
        self.blame_data = Some(result);
    }

    fn load_diff_for_selected(&mut self) {
        let idx = self.timeline.selected().unwrap_or(0);
        if idx >= self.commits.len() {
            return;
        }
        let oid = self.commits[idx].oid.clone();
        let cache_key = format!("diff:{}", oid);
        if let Some(cached) = self.cache.diffs.get(&cache_key) {
            self.diff_panel.load(cached);
            return;
        }
        match compute_commit_diff(&self.repo.repo, &oid) {
            Ok(diff) => {
                self.diff_panel.load(&diff);
                self.cache.diffs.put(cache_key, diff);
            }
            Err(e) => eprintln!("Diff error: {}", e),
        }
    }

    fn load_range_diff(&mut self) {
        let idx = self.timeline.selected().unwrap_or(0);
        if idx >= self.commits.len() {
            return;
        }
        let from_oid = self.commits[idx].oid.clone();
        let to_oid = match self.repo.head_oid() {
            Some(o) => o,
            None => return,
        };
        let cache_key = format!("rdiff:{}:{}", from_oid, to_oid);
        if let Some(cached) = self.cache.diffs.get(&cache_key) {
            self.diff_panel.load(cached);
            return;
        }
        match compute_range_diff(&self.repo.repo, &from_oid, &to_oid) {
            Ok(diff) => {
                self.diff_panel.load(&diff);
                self.cache.diffs.put(cache_key, diff);
            }
            Err(e) => eprintln!("Range diff error: {}", e),
        }
    }

    fn run_search(&mut self) {
        let query = self.search.query.clone();
        if query.is_empty() {
            return;
        }
        self.search.status = format!("Searching for {:?}…", query);
        match &self.search.mode {
            SearchMode::Commits => {
                match self.repo.search_commits(&query, 200) {
                    Ok(results) => {
                        self.search.status = format!("{} results", results.len());
                        self.search.results = results;
                    }
                    Err(e) => self.search.status = format!("Error: {}", e),
                }
            }
            SearchMode::Pickaxe => {
                match self.repo.search_pickaxe(&query, false, 200) {
                    Ok(results) => {
                        self.search.status = format!("{} results", results.len());
                        self.search.results = results;
                    }
                    Err(e) => self.search.status = format!("Error: {}", e),
                }
            }
            SearchMode::Files => {
                // Search by file path in history
                match self.repo.file_history(&query, 200) {
                    Ok(results) => {
                        self.search.status = format!("{} results", results.len());
                        self.search.results = results;
                    }
                    Err(e) => self.search.status = format!("Error: {}", e),
                }
            }
        }
    }

    fn load_tree_for_selected_commit(&mut self) {
        let idx = self.timeline.selected().unwrap_or(0);
        if idx >= self.commits.len() {
            return;
        }
        let oid = self.commits[idx].oid.clone();
        let cache_key = format!("tree:{}", oid);
        if let Some(cached) = self.cache.trees.get(&cache_key) {
            self.root_entries = cached.clone();
            return;
        }
        if let Ok(entries) = tree_at_commit(&self.repo.repo, &oid, "") {
            self.cache.trees.put(cache_key, entries.clone());
            self.root_entries = entries;
        }
    }

    fn expand_tree_entry(&mut self, path: &str) {
        if self.file_tree.is_expanded(path) {
            self.file_tree.toggle_expand(path);
            return;
        }
        let idx = self.timeline.selected().unwrap_or(0);
        if idx >= self.commits.len() {
            return;
        }
        let oid = self.commits[idx].oid.clone();
        match tree_at_commit(&self.repo.repo, &oid, path) {
            Ok(children) => {
                self.child_map.insert(path.to_string(), children);
                self.file_tree.toggle_expand(path);
            }
            Err(e) => eprintln!("Tree expand error: {}", e),
        }
    }

    fn preview_file(&mut self, path: &str) {
        let idx = self.timeline.selected().unwrap_or(0);
        if idx >= self.commits.len() {
            return;
        }
        let oid = self.commits[idx].oid.clone();
        match file_content_at(&self.repo.repo, &oid, path) {
            Ok(content) => {
                self.file_tree.set_preview_highlights(path, &content);
                self.file_tree.file_preview = Some(content);
                self.file_tree.show_preview = true;
                self.file_tree.preview_scroll = 0;
            }
            Err(e) => {
                self.file_tree.file_preview = Some(format!("Error: {}", e));
                self.file_tree.highlighted_preview = None;
                self.file_tree.show_preview = true;
            }
        }
    }

    fn yank_to_clipboard(&self, text: &str) {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
    }

    fn open_in_editor(&self, file_path: &str, line: Option<usize>) {
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());

        // Map file path (repo-relative) to absolute path
        let abs_path = format!("{}/{}", self.repo.path.trim_end_matches('/'), file_path);

        let mut cmd = std::process::Command::new(&editor);
        if let Some(lineno) = line {
            // Many editors accept +N
            cmd.arg(format!("+{}", lineno));
        }
        cmd.arg(&abs_path);

        // Temporarily restore terminal for editor
        if disable_raw_mode().is_ok() {
            if let Ok(mut child) = cmd.spawn() {
                let _ = child.wait();
            }
            let _ = enable_raw_mode();
        }
    }

    fn load_more_commits(&mut self) {
        if !self.has_more_commits {
            return;
        }
        if let Some(last) = self.commits.last() {
            let last_oid = last.oid.clone();
            if let Ok(more) = self.repo.load_commits_after(&last_oid, LOAD_MORE) {
                self.has_more_commits = more.len() == LOAD_MORE;
                self.commits.extend(more);
            }
        }
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<&mut Stdout>>,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key) {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns true if the app should quit
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Help overlay takes over
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return false;
        }

        // Search input mode captures most keys
        if self.active_view == ViewId::Search && self.search.input_active {
            return self.handle_search_input(key);
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('?') => {
                self.show_help = true;
                return false;
            }
            KeyCode::Tab => {
                let next = (self.active_view.index() + 1) % 4;
                self.switch_view(ViewId::from_index(next));
                return false;
            }
            KeyCode::Char('1') => self.switch_view(ViewId::Timeline),
            KeyCode::Char('2') => self.switch_view(ViewId::FileTree),
            KeyCode::Char('3') => self.switch_view(ViewId::Search),
            KeyCode::Char('4') => self.switch_view(ViewId::Blame),
            _ => {}
        }

        match self.active_view {
            ViewId::Timeline => self.handle_timeline_key(key),
            ViewId::FileTree => self.handle_filetree_key(key),
            ViewId::Search => { self.handle_search_nav_key(key); }
            ViewId::Blame => self.handle_blame_key(key),
        }

        false
    }

    fn switch_view(&mut self, view: ViewId) {
        self.active_view = view;
        if view == ViewId::FileTree {
            self.load_tree_for_selected_commit();
        }
    }

    fn handle_timeline_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.commits.len();
                self.timeline.move_down(&self.commits);
                // Load more if near end
                if let Some(idx) = self.timeline.selected() {
                    if idx + 50 >= len {
                        self.load_more_commits();
                    }
                }
                if self.timeline.show_diff {
                    self.load_diff_for_selected();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.timeline.move_up(&self.commits);
                if self.timeline.show_diff {
                    self.load_diff_for_selected();
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.timeline.select(0, self.commits.len());
            }
            KeyCode::Char('G') | KeyCode::End => {
                let len = self.commits.len();
                if len > 0 {
                    self.timeline.select(len - 1, len);
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if key.code == KeyCode::Char('D') {
                    // range diff vs HEAD
                    self.load_range_diff();
                } else {
                    self.load_diff_for_selected();
                }
                self.timeline.show_diff = true;
            }
            KeyCode::Enter => {
                self.timeline.show_diff = !self.timeline.show_diff;
                if self.timeline.show_diff {
                    self.load_diff_for_selected();
                }
            }
            KeyCode::Esc => {
                if self.timeline.show_diff {
                    self.timeline.show_diff = false;
                } else if self.timeline.search_active {
                    self.timeline.search_active = false;
                    self.timeline.search_input.clear();
                }
            }
            KeyCode::Char('/') => {
                self.timeline.search_active = true;
                self.timeline.search_input.clear();
            }
            KeyCode::Char('a') => {
                // Toggle author filter on selected commit
                if let Some(idx) = self.timeline.selected() {
                    if idx < self.commits.len() {
                        let author = self.commits[idx].author.clone();
                        if self.timeline.filter_author == Some(author.clone()) {
                            self.timeline.filter_author = None;
                        } else {
                            self.timeline.filter_author = Some(author);
                        }
                    }
                }
            }
            KeyCode::Char('y') => {
                if let Some(idx) = self.timeline.selected() {
                    if idx < self.commits.len() {
                        let hash = self.commits[idx].oid.clone();
                        self.yank_to_clipboard(&hash);
                    }
                }
            }
            KeyCode::Char('[') => {
                if self.timeline.show_diff {
                    self.diff_panel.scroll_up();
                }
            }
            KeyCode::Char(']') => {
                if self.timeline.show_diff {
                    self.diff_panel.scroll_down();
                }
            }
            KeyCode::Char('o') => {
                // No file context in timeline — do nothing
            }
            KeyCode::PageDown | KeyCode::Char('f') => {
                let cur = self.timeline.selected().unwrap_or(0);
                let new = (cur + 20).min(self.commits.len().saturating_sub(1));
                self.timeline.select(new, self.commits.len());
            }
            KeyCode::PageUp | KeyCode::Char('b') => {
                let cur = self.timeline.selected().unwrap_or(0);
                self.timeline.select(cur.saturating_sub(20), self.commits.len());
            }
            _ => {
                // If search is active, capture character input
                if self.timeline.search_active {
                    match key.code {
                        KeyCode::Char(c) => self.timeline.search_input.push(c),
                        KeyCode::Backspace => { self.timeline.search_input.pop(); }
                        KeyCode::Enter => {
                            // Apply search filter
                            self.timeline.search_active = false;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_filetree_key(&mut self, key: KeyEvent) {
        let flat = build_flat_list(
            &self.root_entries,
            &self.file_tree.expanded,
            &self.child_map,
        );
        let len = flat.len();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.file_tree.move_down(len),
            KeyCode::Char('k') | KeyCode::Up => self.file_tree.move_up(len),
            KeyCode::Enter => {
                if let Some(idx) = self.file_tree.selected() {
                    if idx < flat.len() {
                        let entry = flat[idx].entry.clone();
                        match entry.kind {
                            crate::git::tree::EntryKind::Directory => {
                                self.expand_tree_entry(&entry.path.clone());
                            }
                            _ => {
                                self.preview_file(&entry.path.clone());
                            }
                        }
                    }
                }
            }
            KeyCode::Esc => {
                if self.file_tree.show_preview {
                    self.file_tree.show_preview = false;
                }
            }
            KeyCode::Char('b') => {
                if let Some(idx) = self.file_tree.selected() {
                    if idx < flat.len() {
                        let path = flat[idx].entry.path.clone();
                        self.blame.file_path = path;
                        self.load_blame(None);
                        self.active_view = ViewId::Blame;
                    }
                }
            }
            KeyCode::Char('h') => {
                // Switch to search view and run file history search
                if let Some(idx) = self.file_tree.selected() {
                    if idx < flat.len() {
                        let path = flat[idx].entry.path.clone();
                        self.search.query = path;
                        self.search.mode = SearchMode::Files;
                        self.active_view = ViewId::Search;
                        self.run_search();
                    }
                }
            }
            KeyCode::Char('o') => {
                if let Some(idx) = self.file_tree.selected() {
                    if idx < flat.len() {
                        let path = flat[idx].entry.path.clone();
                        self.open_in_editor(&path, None);
                    }
                }
            }
            KeyCode::Char('y') => {
                if let Some(idx) = self.file_tree.selected() {
                    if idx < flat.len() {
                        let path = flat[idx].entry.path.clone();
                        self.yank_to_clipboard(&path);
                    }
                }
            }
            // Preview scroll
            KeyCode::Char('d') | KeyCode::PageDown => {
                if self.file_tree.show_preview {
                    self.file_tree.scroll_preview_down();
                }
            }
            KeyCode::Char('u') | KeyCode::PageUp => {
                if self.file_tree.show_preview {
                    self.file_tree.scroll_preview_up();
                }
            }
            _ => {}
        }
    }

    fn handle_search_input(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+C or Ctrl+Q to quit
                    if c == 'c' || c == 'q' {
                        return true;
                    }
                } else {
                    self.search.query.push(c);
                }
            }
            KeyCode::Backspace => { self.search.query.pop(); }
            KeyCode::Enter => {
                self.run_search();
                self.search.input_active = false;
            }
            KeyCode::Esc => {
                self.search.input_active = false;
            }
            KeyCode::Tab => {
                self.search.cycle_mode();
            }
            _ => {}
        }
        false
    }

    fn handle_search_nav_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.search.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.search.move_up(),
            KeyCode::Char('/') | KeyCode::Char('i') => {
                self.search.input_active = true;
            }
            KeyCode::Enter => {
                // Jump to commit in timeline
                if let Some(idx) = self.search.selected() {
                    if idx < self.search.results.len() {
                        let oid = self.search.results[idx].oid.clone();
                        self.jump_to_commit(&oid);
                        self.active_view = ViewId::Timeline;
                    }
                }
            }
            KeyCode::Tab => self.search.cycle_mode(),
            _ => {}
        }
        false
    }

    fn handle_blame_key(&mut self, key: KeyEvent) {
        let len = self
            .blame_data
            .as_ref()
            .map(|b| b.lines.len())
            .unwrap_or(0);

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.blame.move_down(len),
            KeyCode::Char('k') | KeyCode::Up => self.blame.move_up(len),
            KeyCode::Char('g') | KeyCode::Home => {
                self.blame.list_state.select(Some(0));
            }
            KeyCode::Char('G') | KeyCode::End => {
                if len > 0 {
                    self.blame.list_state.select(Some(len - 1));
                }
            }
            KeyCode::Char('d') | KeyCode::PageDown => {
                let page = 20;
                self.blame.page_down(len, page);
            }
            KeyCode::Char('u') | KeyCode::PageUp => {
                self.blame.page_up(20);
            }
            KeyCode::Enter => {
                // Jump to commit in timeline
                if let (Some(idx), Some(blame)) =
                    (self.blame.selected(), self.blame_data.as_ref())
                {
                    if idx < blame.lines.len() {
                        let oid = blame.lines[idx].commit_oid.clone();
                        self.jump_to_commit(&oid);
                        self.active_view = ViewId::Timeline;
                    }
                }
            }
            KeyCode::Char('p') => {
                // Blame at parent commit
                let parent_info: Option<(String, String)> = self
                    .blame_data
                    .as_ref()
                    .and_then(|blame| {
                        let idx = self.blame.selected()?;
                        let line = blame.lines.get(idx)?;
                        let oid_str = line.commit_oid.clone();
                        let file = blame.file_path.clone();
                        Some((oid_str, file))
                    });

                if let Some((oid_str, file)) = parent_info {
                    let parent_oid_opt: Option<String> = git2::Oid::from_str(&oid_str)
                        .ok()
                        .and_then(|oid| self.repo.repo.find_commit(oid).ok())
                        .and_then(|commit| commit.parent_id(0).ok())
                        .map(|pid| pid.to_string());

                    if let Some(parent_oid) = parent_oid_opt {
                        self.blame.file_path = file;
                        self.load_blame(Some(&parent_oid));
                    }
                }
            }
            KeyCode::Char('y') => {
                if let (Some(idx), Some(blame)) =
                    (self.blame.selected(), self.blame_data.as_ref())
                {
                    if idx < blame.lines.len() {
                        let hash = blame.lines[idx].commit_oid.clone();
                        self.yank_to_clipboard(&hash);
                    }
                }
            }
            KeyCode::Char('o') => {
                if let (Some(idx), Some(blame)) =
                    (self.blame.selected(), self.blame_data.as_ref())
                {
                    if idx < blame.lines.len() {
                        let file = blame.file_path.clone();
                        let lineno = blame.lines[idx].lineno;
                        self.open_in_editor(&file, Some(lineno));
                    }
                }
            }
            _ => {}
        }
    }

    fn jump_to_commit(&mut self, oid: &str) {
        if let Some(pos) = self.commits.iter().position(|c| c.oid == oid) {
            self.timeline.select(pos, self.commits.len());
        }
    }

    fn render(&mut self, f: &mut ratatui::Frame) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // top bar
                Constraint::Min(0),    // content
                Constraint::Length(1), // bottom bar
            ])
            .split(area);

        self.render_top_bar(f, chunks[0]);
        self.render_content(f, chunks[1]);
        self.render_bottom_bar(f, chunks[2]);

        if self.show_help {
            render_help_overlay(f, area);
        }
    }

    fn render_top_bar(&self, f: &mut ratatui::Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(30), Constraint::Length(50)])
            .split(area);

        // Repo name + branch
        let repo_text = format!(
            " {} ⎇ {}",
            self.repo.name(),
            self.repo.current_branch()
        );
        let repo_para = Paragraph::new(repo_text)
            .style(Style::default().fg(Theme::ACCENT).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(Theme::BORDER)),
            );
        f.render_widget(repo_para, chunks[0]);

        // Tabs
        let tab_titles = vec!["[1] Timeline", "[2] Files", "[3] Search", "[4] Blame"];
        let tabs = Tabs::new(tab_titles)
            .select(self.active_view.index())
            .style(Style::default().fg(Theme::TAB_INACTIVE))
            .highlight_style(
                Style::default()
                    .fg(Theme::TAB_ACTIVE)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│")
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(Theme::BORDER)),
            );
        f.render_widget(tabs, chunks[1]);
    }

    fn render_content(&mut self, f: &mut ratatui::Frame, area: Rect) {
        match self.active_view {
            ViewId::Timeline => {
                // Build filtered commits list for rendering
                let filter_q = self.timeline.search_input.clone();
                let filter_author = self.timeline.filter_author.clone();
                let filter_lower = filter_q.to_lowercase();

                let filtered: Vec<CommitInfo> = self
                    .commits
                    .iter()
                    .filter(|c| {
                        let author_ok = filter_author
                            .as_ref()
                            .map_or(true, |a| c.author.contains(a.as_str()));
                        let query_ok = if filter_lower.is_empty() {
                            true
                        } else {
                            c.summary.to_lowercase().contains(&filter_lower)
                                || c.author.to_lowercase().contains(&filter_lower)
                                || c.oid.starts_with(&filter_lower)
                        };
                        author_ok && query_ok
                    })
                    .cloned()
                    .collect();

                let diff_panel = if self.timeline.show_diff {
                    Some(&mut self.diff_panel)
                } else {
                    None
                };

                render_timeline(f, area, &filtered, &mut self.timeline, diff_panel);
            }
            ViewId::FileTree => {
                let flat = build_flat_list(
                    &self.root_entries,
                    &self.file_tree.expanded,
                    &self.child_map,
                );
                let commit_short = self
                    .commits
                    .get(self.timeline.selected().unwrap_or(0))
                    .map(|c| c.short_hash.clone())
                    .unwrap_or_else(|| "HEAD".to_string());
                render_filetree(f, area, &flat, &mut self.file_tree, &commit_short);
            }
            ViewId::Search => {
                render_search(f, area, &mut self.search);
            }
            ViewId::Blame => {
                render_blame(f, area, self.blame_data.as_ref(), &mut self.blame);
            }
        }
    }

    fn render_bottom_bar(&self, f: &mut ratatui::Frame, area: Rect) {
        let hints = match self.active_view {
            ViewId::Timeline => {
                " j/k: nav  Enter: diff  d: diff  D: range diff  [/]: scroll diff  /: search  a: author  y: yank  ?: help  q: quit"
            }
            ViewId::FileTree => {
                " j/k: nav  Enter: expand/preview  b: blame  h: history  o: editor  y: yank  ?: help  q: quit"
            }
            ViewId::Search => {
                " /: edit query  Tab: mode  Enter: search/jump  j/k: nav  ?: help  q: quit"
            }
            ViewId::Blame => {
                " j/k: nav  Enter: jump to commit  p: parent blame  o: editor  y: yank  ?: help  q: quit"
            }
        };
        let para = Paragraph::new(hints)
            .style(Style::default().fg(Theme::DIM).bg(Theme::SURFACE));
        f.render_widget(para, area);
    }
}
