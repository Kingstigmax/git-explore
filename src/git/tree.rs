use anyhow::{Context, Result};
use git2::{Oid, Repository};

#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

pub fn tree_at_commit(repo: &Repository, oid_str: &str, prefix: &str) -> Result<Vec<TreeEntry>> {
    let oid = Oid::from_str(oid_str)?;
    let commit = repo
        .find_commit(oid)
        .with_context(|| format!("Commit not found: {}", oid_str))?;
    let tree = commit.tree()?;

    let subtree = if prefix.is_empty() {
        tree
    } else {
        let entry = tree
            .get_path(std::path::Path::new(prefix))
            .with_context(|| format!("Path not found: {}", prefix))?;
        let obj = entry.to_object(repo)?;
        obj.into_tree()
            .map_err(|_| anyhow::anyhow!("Path is not a directory: {}", prefix))?
    };

    let mut entries: Vec<TreeEntry> = Vec::new();

    for entry in subtree.iter() {
        let name = entry.name().unwrap_or("").to_string();
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        let kind = match entry.kind() {
            Some(git2::ObjectType::Tree) => EntryKind::Directory,
            Some(git2::ObjectType::Blob) => {
                // Check for symlink via filemode
                if entry.filemode() == 0o120000 {
                    EntryKind::Symlink
                } else {
                    EntryKind::File
                }
            }
            _ => EntryKind::File,
        };

        let size = if kind == EntryKind::File {
            repo.find_blob(entry.id()).ok().map(|b| b.size() as u64)
        } else {
            None
        };

        entries.push(TreeEntry { name, path, kind, size });
    }

    // Directories first, then files, both sorted by name
    entries.sort_by(|a, b| {
        let a_is_dir = a.kind == EntryKind::Directory;
        let b_is_dir = b.kind == EntryKind::Directory;
        b_is_dir
            .cmp(&a_is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

pub fn file_content_at(repo: &Repository, oid_str: &str, file_path: &str) -> Result<String> {
    let oid = Oid::from_str(oid_str)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let entry = tree
        .get_path(std::path::Path::new(file_path))
        .with_context(|| format!("File not found: {}", file_path))?;
    let blob = repo.find_blob(entry.id())?;
    Ok(String::from_utf8_lossy(blob.content()).to_string())
}
