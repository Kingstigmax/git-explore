use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use git2::{BranchType, Oid, Repository, Sort};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct CommitInfo {
    pub oid: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub time: DateTime<Utc>,
    pub message: String,
    pub summary: String,
    pub refs: Vec<String>,
    pub parent_oids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct CommitDetail {
    pub info: CommitInfo,
    pub body: String,
    pub stats: DiffStats,
}

#[derive(Clone, Debug, Default)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

pub struct GitRepo {
    pub repo: Repository,
    pub path: String,
}

impl GitRepo {
    pub fn open(path: &str) -> Result<Self> {
        let repo = Repository::discover(path)
            .with_context(|| format!("Not a git repository: {}", path))?;
        let workdir = repo
            .workdir()
            .or_else(|| Some(repo.path()))
            .unwrap_or(Path::new(path));
        let path_str = workdir.to_string_lossy().to_string();
        Ok(Self { repo, path: path_str })
    }

    pub fn name(&self) -> String {
        Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string())
    }

    pub fn current_branch(&self) -> String {
        self.repo
            .head()
            .ok()
            .and_then(|h| {
                if h.is_branch() {
                    h.shorthand().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "HEAD".to_string())
    }

    pub fn head_oid(&self) -> Option<String> {
        self.repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .map(|oid| oid.to_string())
    }

    /// Build a ref map: oid -> list of ref labels
    pub fn build_ref_map(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        // Branches
        if let Ok(branches) = self.repo.branches(None) {
            for branch in branches.flatten() {
                let (b, btype) = branch;
                if let Some(name) = b.name().ok().flatten() {
                    let label = match btype {
                        BranchType::Remote => format!("remote/{}", name),
                        BranchType::Local => name.to_string(),
                    };
                    if let Some(oid) = b.get().target() {
                        map.entry(oid.to_string()).or_default().push(label);
                    }
                }
            }
        }

        // Tags
        let _ = self.repo.tag_foreach(|oid, name| {
            let tag_name = String::from_utf8_lossy(name)
                .trim_start_matches("refs/tags/")
                .to_string();
            // Dereference tag objects to commit oids
            let target_oid = self
                .repo
                .find_tag(oid)
                .ok()
                .and_then(|t| t.target().ok())
                .map(|obj| obj.id())
                .unwrap_or(oid);
            map.entry(target_oid.to_string())
                .or_default()
                .push(format!("tag:{}", tag_name));
            true
        });

        map
    }

    /// Load commits from HEAD, up to `limit`. Returns commits newest-first.
    pub fn load_commits(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TIME)?;
        revwalk.push_head()?;

        let ref_map = self.build_ref_map();
        let mut commits = Vec::with_capacity(limit.min(1000));

        for oid_result in revwalk.take(limit) {
            let oid = oid_result?;
            if let Ok(commit) = self.repo.find_commit(oid) {
                commits.push(commit_to_info(&commit, &ref_map));
            }
        }

        Ok(commits)
    }

    /// Load more commits starting after `after_oid`.
    pub fn load_commits_after(&self, after_oid: &str, limit: usize) -> Result<Vec<CommitInfo>> {
        let start = Oid::from_str(after_oid)?;
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TIME)?;
        revwalk.push_head()?;

        let ref_map = self.build_ref_map();
        let mut found = false;
        let mut commits = Vec::with_capacity(limit);

        for oid_result in revwalk {
            let oid = oid_result?;
            if !found {
                if oid == start {
                    found = true;
                }
                continue;
            }
            if let Ok(commit) = self.repo.find_commit(oid) {
                commits.push(commit_to_info(&commit, &ref_map));
            }
            if commits.len() >= limit {
                break;
            }
        }

        Ok(commits)
    }

    pub fn commit_detail(&self, oid_str: &str) -> Result<CommitDetail> {
        let oid = Oid::from_str(oid_str)?;
        let commit = self.repo.find_commit(oid)?;
        let ref_map = self.build_ref_map();
        let info = commit_to_info(&commit, &ref_map);

        let body = commit
            .body()
            .unwrap_or("")
            .to_string();

        // Compute diff stats
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let commit_tree = commit.tree().ok();
        let stats = match (commit_tree.as_ref(), parent_tree.as_ref()) {
            (Some(ct), Some(pt)) => {
                let diff = self
                    .repo
                    .diff_tree_to_tree(Some(pt), Some(ct), None)
                    .ok();
                diff.and_then(|d| d.stats().ok())
                    .map(|s| DiffStats {
                        files_changed: s.files_changed(),
                        insertions: s.insertions(),
                        deletions: s.deletions(),
                    })
                    .unwrap_or_default()
            }
            (Some(ct), None) => {
                // Initial commit — diff against empty tree
                let diff = self
                    .repo
                    .diff_tree_to_tree(None, Some(ct), None)
                    .ok();
                diff.and_then(|d| d.stats().ok())
                    .map(|s| DiffStats {
                        files_changed: s.files_changed(),
                        insertions: s.insertions(),
                        deletions: s.deletions(),
                    })
                    .unwrap_or_default()
            }
            _ => DiffStats::default(),
        };

        Ok(CommitDetail { info, body, stats })
    }

    /// Search commits matching a query (message, author, or hash prefix).
    pub fn search_commits(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<CommitInfo>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TIME)?;
        revwalk.push_head()?;

        let query_lower = query.to_lowercase();
        let ref_map = self.build_ref_map();
        let mut results = Vec::new();

        for oid_result in revwalk {
            let oid = oid_result?;
            if let Ok(commit) = self.repo.find_commit(oid) {
                let hash = oid.to_string();
                let author = commit.author().name().unwrap_or("").to_lowercase();
                let msg = commit.message().unwrap_or("").to_lowercase();

                if hash.starts_with(&query_lower)
                    || author.contains(&query_lower)
                    || msg.contains(&query_lower)
                {
                    results.push(commit_to_info(&commit, &ref_map));
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    /// Search for a string in file contents across history (like git log -S).
    pub fn search_pickaxe(
        &self,
        pattern: &str,
        use_regex: bool,
        max_results: usize,
    ) -> Result<Vec<CommitInfo>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TIME)?;
        revwalk.push_head()?;

        let ref_map = self.build_ref_map();
        let mut results = Vec::new();

        let regex = if use_regex {
            Some(regex_simple::Regex::new(pattern))
        } else {
            None
        };

        for oid_result in revwalk {
            let oid = oid_result?;
            let commit = match self.repo.find_commit(oid) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let commit_tree = match commit.tree() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

            let diff = match self
                .repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)
            {
                Ok(d) => d,
                Err(_) => continue,
            };

            let mut matched = false;
            let _ = diff.foreach(
                &mut |_, _| true,
                None,
                None,
                Some(&mut |_delta, _hunk, line| {
                    if matched {
                        return true;
                    }
                    let content = std::str::from_utf8(line.content()).unwrap_or("");
                    let hit = if let Some(ref re) = regex {
                        re.is_match(content)
                    } else {
                        content.contains(pattern)
                    };
                    if hit {
                        matched = true;
                    }
                    true
                }),
            );

            if matched {
                results.push(commit_to_info(&commit, &ref_map));
                if results.len() >= max_results {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Find all commits that touched a specific file path.
    pub fn file_history(&self, file_path: &str, max_results: usize) -> Result<Vec<CommitInfo>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TIME)?;
        revwalk.push_head()?;

        let ref_map = self.build_ref_map();
        let mut results = Vec::new();

        for oid_result in revwalk {
            let oid = oid_result?;
            let commit = match self.repo.find_commit(oid) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let commit_tree = match commit.tree() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

            let mut opts = git2::DiffOptions::new();
            opts.pathspec(file_path);

            let diff = match self
                .repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), Some(&mut opts))
            {
                Ok(d) => d,
                Err(_) => continue,
            };

            if diff.deltas().len() > 0 {
                results.push(commit_to_info(&commit, &ref_map));
                if results.len() >= max_results {
                    break;
                }
            }
        }

        Ok(results)
    }
}

fn commit_to_info(commit: &git2::Commit, ref_map: &HashMap<String, Vec<String>>) -> CommitInfo {
    let oid = commit.id().to_string();
    let short_hash = oid[..8].to_string();
    let author_sig = commit.author();
    let author = author_sig.name().unwrap_or("unknown").to_string();
    let email = author_sig.email().unwrap_or("").to_string();
    let ts = author_sig.when().seconds();
    let time = Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now);
    let message = commit.message().unwrap_or("").to_string();
    let summary = commit
        .summary()
        .unwrap_or("")
        .to_string();
    let refs = ref_map.get(&oid).cloned().unwrap_or_default();
    let parent_oids = (0..commit.parent_count())
        .filter_map(|i| commit.parent_id(i).ok())
        .map(|o| o.to_string())
        .collect();

    CommitInfo {
        oid,
        short_hash,
        author,
        email,
        time,
        message,
        summary,
        refs,
        parent_oids,
    }
}

/// Minimal regex engine stub — replaced by simple string contains for non-regex.
/// We avoid pulling in the `regex` crate to keep deps slim; for regex search
/// we do a basic contains check here and note in the UI.
mod regex_simple {
    pub struct Regex(String);
    impl Regex {
        pub fn new(pattern: &str) -> Self {
            Self(pattern.to_string())
        }
        pub fn is_match(&self, text: &str) -> bool {
            text.contains(&self.0)
        }
    }
}
