//! The in-memory board: the list of tips plus pure update operations.
//! All functions return a new `Board`; nothing is mutated in place.

use std::collections::HashMap;

use crate::core::tip::Tip;
use crate::sync::github::RemoteFile;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Board {
    pub tips: Vec<Tip>,
}

/// What a sync needs to download, computed from the remote listing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncPlan {
    /// Paths whose SHA is new or changed.
    pub to_fetch: Vec<String>,
    /// Paths that no longer exist remotely.
    pub removed: Vec<String>,
}

impl Board {
    pub fn new(tips: Vec<Tip>) -> Self {
        Self { tips }
    }

    pub fn get(&self, id: &str) -> Option<&Tip> {
        self.tips.iter().find(|t| t.id == id)
    }

    pub fn plan(&self, remote: &[RemoteFile]) -> SyncPlan {
        let known: HashMap<&str, &str> = self
            .tips
            .iter()
            .map(|t| (t.path.as_str(), t.sha.as_deref().unwrap_or("")))
            .collect();
        let remote_md: Vec<&RemoteFile> = remote
            .iter()
            .filter(|f| f.kind == "file" && f.name.ends_with(".md"))
            .collect();
        let to_fetch = remote_md
            .iter()
            .filter(|f| known.get(f.path.as_str()) != Some(&f.sha.as_str()))
            .map(|f| f.path.clone())
            .collect();
        let remote_paths: std::collections::HashSet<&str> =
            remote_md.iter().map(|f| f.path.as_str()).collect();
        let removed = self
            .tips
            .iter()
            .filter(|t| !remote_paths.contains(t.path.as_str()))
            .map(|t| t.path.clone())
            .collect();
        SyncPlan { to_fetch, removed }
    }

    /// Apply fetched tips and removals. Returns the new board and the ids of
    /// tips that did not exist before (candidates for a "new tip" toast).
    pub fn apply(&self, fetched: Vec<Tip>, removed: &[String]) -> (Board, Vec<String>) {
        let fetched_paths: std::collections::HashSet<String> =
            fetched.iter().map(|t| t.path.clone()).collect();
        let new_ids = fetched
            .iter()
            .filter(|t| self.get(&t.id).is_none())
            .map(|t| t.id.clone())
            .collect();
        let kept: Vec<Tip> = self
            .tips
            .iter()
            .filter(|t| !removed.contains(&t.path) && !fetched_paths.contains(&t.path))
            .cloned()
            .collect();
        let tips = kept.into_iter().chain(fetched).collect();
        (Board { tips }, new_ids)
    }

    /// Replace one tip by id (or append if missing).
    pub fn upsert(&self, tip: Tip) -> Board {
        let mut found = false;
        let mut tips: Vec<Tip> = self
            .tips
            .iter()
            .map(|t| {
                if t.id == tip.id {
                    found = true;
                    tip.clone()
                } else {
                    t.clone()
                }
            })
            .collect();
        if !found {
            tips.push(tip);
        }
        Board { tips }
    }

    /// A file name that does not collide with an existing tip.
    pub fn unique_file_name(&self, stem: &str) -> String {
        let ids: std::collections::HashSet<&str> =
            self.tips.iter().map(|t| t.id.as_str()).collect();
        if !ids.contains(stem) {
            return format!("{stem}.md");
        }
        (2..)
            .map(|n| format!("{stem}-{n}"))
            .find(|c| !ids.contains(c.as_str()))
            .map(|c| format!("{c}.md"))
            .expect("unbounded iterator")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip(path: &str, sha: &str) -> Tip {
        Tip::parse(path, Some(sha.into()), "---\ntitle: T\n---\n").unwrap()
    }
    fn remote(path: &str, sha: &str) -> RemoteFile {
        RemoteFile {
            name: path.rsplit('/').next().unwrap().into(),
            path: path.into(),
            sha: sha.into(),
            size: 1,
            kind: "file".into(),
        }
    }

    #[test]
    fn plan_detects_new_changed_removed_and_ignores_non_md() {
        let board = Board::new(vec![
            tip("tips/a.md", "1"),
            tip("tips/b.md", "2"),
            tip("tips/c.md", "3"),
        ]);
        let listing = vec![
            remote("tips/a.md", "1"),
            remote("tips/b.md", "changed"),
            remote("tips/d.md", "4"),
            remote("tips/figures", "x"),
            remote("tips/readme.txt", "y"),
        ];
        let plan = board.plan(&listing);
        assert_eq!(plan.to_fetch, vec!["tips/b.md", "tips/d.md"]);
        assert_eq!(plan.removed, vec!["tips/c.md"]);
    }

    #[test]
    fn apply_merges_and_reports_new_ids() {
        let board = Board::new(vec![tip("tips/a.md", "1"), tip("tips/b.md", "2")]);
        let (next, new_ids) = board.apply(
            vec![tip("tips/b.md", "9"), tip("tips/d.md", "4")],
            &["tips/a.md".into()],
        );
        let mut ids: Vec<&str> = next.tips.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["b", "d"]);
        assert_eq!(next.get("b").unwrap().sha.as_deref(), Some("9"));
        assert_eq!(new_ids, vec!["d"]);
        assert_eq!(board.tips.len(), 2, "original untouched");
    }

    #[test]
    fn upsert_and_unique_names() {
        let board = Board::new(vec![tip("tips/a.md", "1")]);
        let next = board
            .upsert(tip("tips/a.md", "2"))
            .upsert(tip("tips/z.md", "3"));
        assert_eq!(next.tips.len(), 2);
        assert_eq!(next.get("a").unwrap().sha.as_deref(), Some("2"));
        assert_eq!(next.unique_file_name("a"), "a-2.md");
        assert_eq!(next.unique_file_name("q"), "q.md");
        let more = next.upsert(tip("tips/a-2.md", "4"));
        assert_eq!(more.unique_file_name("a"), "a-3.md");
    }
}
