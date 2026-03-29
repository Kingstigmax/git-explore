use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use git2::{BlameOptions, Oid, Repository};

#[derive(Clone, Debug)]
pub struct BlameLine {
    pub lineno: usize,
    pub content: String,
    pub commit_oid: String,
    pub short_hash: String,
    pub author: String,
    pub time: DateTime<Utc>,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct BlameResult {
    pub file_path: String,
    pub lines: Vec<BlameLine>,
    pub oldest_time: DateTime<Utc>,
    pub newest_time: DateTime<Utc>,
}

pub fn compute_blame(repo: &Repository, file_path: &str) -> Result<BlameResult> {
    compute_blame_at(repo, file_path, None)
}

pub fn compute_blame_at(
    repo: &Repository,
    file_path: &str,
    commit_oid: Option<&str>,
) -> Result<BlameResult> {
    let mut opts = BlameOptions::new();
    if let Some(oid_str) = commit_oid {
        let oid = Oid::from_str(oid_str)?;
        opts.newest_commit(oid);
    }

    let blame = repo
        .blame_file(std::path::Path::new(file_path), Some(&mut opts))
        .with_context(|| format!("Could not blame file: {}", file_path))?;

    // Read the file content at the given commit (or HEAD)
    let file_content = if let Some(oid_str) = commit_oid {
        let oid = Oid::from_str(oid_str)?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let entry = tree
            .get_path(std::path::Path::new(file_path))
            .with_context(|| format!("File not found in tree: {}", file_path))?;
        let blob = repo.find_blob(entry.id())?;
        String::from_utf8_lossy(blob.content()).to_string()
    } else {
        let head = repo.head()?;
        let commit = repo.find_commit(head.target().context("HEAD has no target")?)?;
        let tree = commit.tree()?;
        let entry = tree
            .get_path(std::path::Path::new(file_path))
            .with_context(|| format!("File not found in tree: {}", file_path))?;
        let blob = repo.find_blob(entry.id())?;
        String::from_utf8_lossy(blob.content()).to_string()
    };

    let file_lines: Vec<&str> = file_content.lines().collect();
    let mut blame_lines = Vec::new();

    for (idx, line_content) in file_lines.iter().enumerate() {
        let lineno = idx + 1;
        if let Some(hunk) = blame.get_line(lineno) {
            let sig = hunk.final_signature();
            let ts = sig.when().seconds();
            let time = Utc
                .timestamp_opt(ts, 0)
                .single()
                .unwrap_or_else(Utc::now);
            let oid = hunk.final_commit_id();
            let oid_str = oid.to_string();
            let short_hash = oid_str[..8].to_string();
            let author = sig.name().unwrap_or("unknown").to_string();
            let summary = repo
                .find_commit(oid)
                .ok()
                .and_then(|c| c.summary().map(|s| s.to_string()))
                .unwrap_or_default();

            blame_lines.push(BlameLine {
                lineno,
                content: line_content.to_string(),
                commit_oid: oid_str,
                short_hash,
                author,
                time,
                summary,
            });
        }
    }

    let oldest_time = blame_lines
        .iter()
        .map(|l| l.time)
        .min()
        .unwrap_or_else(Utc::now);
    let newest_time = blame_lines
        .iter()
        .map(|l| l.time)
        .max()
        .unwrap_or_else(Utc::now);

    Ok(BlameResult {
        file_path: file_path.to_string(),
        lines: blame_lines,
        oldest_time,
        newest_time,
    })
}
