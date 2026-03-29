use lru::LruCache;
use std::num::NonZeroUsize;

use crate::git::{BlameResult, CommitDetail, DiffResult, TreeEntry};

const DIFF_CACHE_SIZE: usize = 50;
const BLAME_CACHE_SIZE: usize = 20;
const TREE_CACHE_SIZE: usize = 30;
const DETAIL_CACHE_SIZE: usize = 200;

pub struct AppCache {
    pub diffs: LruCache<String, DiffResult>,
    pub blames: LruCache<String, BlameResult>,
    pub trees: LruCache<String, Vec<TreeEntry>>,
    pub details: LruCache<String, CommitDetail>,
}

impl AppCache {
    pub fn new() -> Self {
        Self {
            diffs: LruCache::new(NonZeroUsize::new(DIFF_CACHE_SIZE).unwrap()),
            blames: LruCache::new(NonZeroUsize::new(BLAME_CACHE_SIZE).unwrap()),
            trees: LruCache::new(NonZeroUsize::new(TREE_CACHE_SIZE).unwrap()),
            details: LruCache::new(NonZeroUsize::new(DETAIL_CACHE_SIZE).unwrap()),
        }
    }
}
