//! Path-filter helpers applied to a changeset or to enumeration stubs.

use super::types::FileStub;
use crate::model::Changeset;

/// Keep only files matching one of the given path filters (repo-root-relative
/// pathspecs). An empty filter list keeps everything. Renames match on either
/// side. Returns the count removed.
pub fn apply_path_filter(cs: &mut Changeset, filters: &[String]) {
    if filters.is_empty() {
        return;
    }
    cs.files.retain(|f| {
        filters.iter().any(|spec| {
            path_matches(&f.path, spec)
                || f.previous_path
                    .as_deref()
                    .is_some_and(|p| path_matches(p, spec))
        })
    });
}

/// Like [`apply_path_filter`] but over enumeration stubs (before diffing), so a
/// streaming load only diffs the files that pass the filter.
pub fn apply_stub_filter(stubs: &mut Vec<FileStub>, filters: &[String]) {
    if filters.is_empty() {
        return;
    }
    stubs.retain(|s| {
        filters.iter().any(|spec| {
            path_matches(&s.path, spec)
                || s.previous_path
                    .as_deref()
                    .is_some_and(|p| path_matches(p, spec))
        })
    });
}

fn path_matches(path: &str, spec: &str) -> bool {
    let spec = spec.trim_end_matches('/');
    path == spec || path.starts_with(&format!("{spec}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::types::Side;
    use crate::model::FileStatus;

    fn stub(path: &str) -> FileStub {
        FileStub {
            path: path.into(),
            previous_path: None,
            status: FileStatus::Modified,
            staged: false,
            language: None,
            old: Side::Absent,
            new: Side::Absent,
        }
    }

    fn renamed(path: &str, previous: &str) -> FileStub {
        FileStub {
            previous_path: Some(previous.into()),
            status: FileStatus::Renamed,
            ..stub(path)
        }
    }

    fn paths(stubs: &[FileStub]) -> Vec<&str> {
        stubs.iter().map(|s| s.path.as_str()).collect()
    }

    #[test]
    fn empty_filter_keeps_everything() {
        // The early return: an empty filter list is a no-op, every stub survives.
        let mut stubs = vec![stub("src/a.rs"), stub("README.md")];
        apply_stub_filter(&mut stubs, &[]);
        assert_eq!(paths(&stubs), ["src/a.rs", "README.md"]);
    }

    #[test]
    fn exact_path_match_survives_others_removed() {
        let mut stubs = vec![stub("src/a.rs"), stub("src/b.rs"), stub("README.md")];
        apply_stub_filter(&mut stubs, &["src/a.rs".to_string()]);
        assert_eq!(paths(&stubs), ["src/a.rs"]);
    }

    #[test]
    fn directory_prefix_keeps_subtree_only() {
        // A directory pathspec keeps files under it (prefix + "/"), drops the rest,
        // and a trailing slash on the spec is tolerated (trim_end_matches).
        let mut stubs = vec![
            stub("src/a.rs"),
            stub("src/tui/app.rs"),
            stub("lib/b.rs"),
            // A path that shares the spec as a string prefix but is NOT under the
            // directory ("src2" vs "src/") must not match.
            stub("src2/c.rs"),
        ];
        apply_stub_filter(&mut stubs, &["src/".to_string()]);
        assert_eq!(paths(&stubs), ["src/a.rs", "src/tui/app.rs"]);
    }

    #[test]
    fn rename_matches_on_previous_path() {
        // A renamed file passes the filter when only its old (previous) path is
        // under the filtered directory — the `previous_path` arm of `any`.
        let mut stubs = vec![renamed("new/here.rs", "old/there.rs"), stub("unrelated.rs")];
        apply_stub_filter(&mut stubs, &["old".to_string()]);
        assert_eq!(paths(&stubs), ["new/here.rs"]);
    }

    #[test]
    fn no_match_removes_all() {
        let mut stubs = vec![stub("src/a.rs"), stub("src/b.rs")];
        apply_stub_filter(&mut stubs, &["docs".to_string()]);
        assert!(stubs.is_empty());
    }
}
