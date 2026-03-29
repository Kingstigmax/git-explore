use anyhow::Result;
use git2::{DiffFormat, Oid, Repository};

#[derive(Clone, Debug)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub origin: char, // '+', '-', ' ', '\'
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: char, // 'A', 'D', 'M', 'R', etc.
    pub hunks: Vec<DiffHunk>,
}

#[derive(Clone, Debug)]
pub struct DiffResult {
    pub files: Vec<FileDiff>,
    pub stats_text: String,
}

pub fn compute_commit_diff(repo: &Repository, oid_str: &str) -> Result<DiffResult> {
    let oid = Oid::from_str(oid_str)?;
    let commit = repo.find_commit(oid)?;
    let commit_tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), Some(&mut opts))?;
    parse_diff(repo, &diff)
}

pub fn compute_range_diff(
    repo: &Repository,
    from_oid: &str,
    to_oid: &str,
) -> Result<DiffResult> {
    let from = Oid::from_str(from_oid)?;
    let to = Oid::from_str(to_oid)?;
    let from_commit = repo.find_commit(from)?;
    let to_commit = repo.find_commit(to)?;
    let from_tree = from_commit.tree()?;
    let to_tree = to_commit.tree()?;

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);

    let diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut opts))?;
    parse_diff(repo, &diff)
}

pub fn compute_file_diff_at(
    repo: &Repository,
    oid_str: &str,
    file_path: &str,
) -> Result<DiffResult> {
    let oid = Oid::from_str(oid_str)?;
    let commit = repo.find_commit(oid)?;
    let commit_tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);
    opts.pathspec(file_path);

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), Some(&mut opts))?;
    parse_diff(repo, &diff)
}

fn parse_diff(repo: &Repository, diff: &git2::Diff) -> Result<DiffResult> {
    let stats = diff.stats()?;
    let stats_text = format!(
        "{} file(s) changed, {} insertion(s), {} deletion(s)",
        stats.files_changed(),
        stats.insertions(),
        stats.deletions()
    );

    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunk: Option<DiffHunk> = None;

    diff.print(DiffFormat::Patch, |delta, hunk_opt, line| {
        // New delta (file)
        let new_path = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
        let old_path = delta.old_file().path().map(|p| p.to_string_lossy().to_string());

        let is_new_file = current_file.as_ref().map_or(true, |f| {
            f.new_path != new_path || f.old_path != old_path
        });

        if is_new_file {
            // Flush current hunk into current file
            if let Some(hunk) = current_hunk.take() {
                if let Some(ref mut cf) = current_file {
                    cf.hunks.push(hunk);
                }
            }
            // Flush current file
            if let Some(cf) = current_file.take() {
                files.push(cf);
            }

            let status = match delta.status() {
                git2::Delta::Added => 'A',
                git2::Delta::Deleted => 'D',
                git2::Delta::Modified => 'M',
                git2::Delta::Renamed => 'R',
                git2::Delta::Copied => 'C',
                _ => '?',
            };

            current_file = Some(FileDiff {
                old_path,
                new_path,
                status,
                hunks: Vec::new(),
            });
        }

        // New hunk
        if let Some(hunk) = hunk_opt {
            let header = std::str::from_utf8(hunk.header()).unwrap_or("").trim_end().to_string();
            let is_new_hunk = current_hunk.as_ref().map_or(true, |h| h.header != header);
            if is_new_hunk {
                if let Some(h) = current_hunk.take() {
                    if let Some(ref mut cf) = current_file {
                        cf.hunks.push(h);
                    }
                }
                current_hunk = Some(DiffHunk {
                    header,
                    lines: Vec::new(),
                });
            }
        }

        let origin = line.origin();
        if matches!(origin, '+' | '-' | ' ' | '\\') {
            if let Some(ref mut hunk) = current_hunk {
                let content = std::str::from_utf8(line.content())
                    .unwrap_or("")
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_string();
                hunk.lines.push(DiffLine {
                    origin,
                    content,
                    old_lineno: line.old_lineno(),
                    new_lineno: line.new_lineno(),
                });
            }
        }

        true
    })?;

    // Flush remaining
    if let Some(hunk) = current_hunk.take() {
        if let Some(ref mut cf) = current_file {
            cf.hunks.push(hunk);
        }
    }
    if let Some(cf) = current_file {
        files.push(cf);
    }

    let _ = repo; // suppress unused warning
    Ok(DiffResult { files, stats_text })
}
