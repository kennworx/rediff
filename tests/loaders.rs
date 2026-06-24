//! Integration tests for the git loaders against a real temp repo.

mod common;

use common::GitFixture;
use rediff::git::{self, LoadRequest};
use rediff::model::FileStatus;

/// Build a repo: one commit, then a staged mod, an unstaged mod, a staged
/// rename+edit, and an untracked file.
fn populated() -> GitFixture {
    let f = GitFixture::new();
    f.write("a.rs", "fn main() {\n    one();\n}\n");
    f.write("b.rs", "pub fn b() -> i32 {\n    1\n}\n");
    f.write(
        "notes.md",
        "line one\nline two\nline three\nline four\nline five\n",
    );
    f.commit_all("init");

    // staged modification to b.rs
    f.write("b.rs", "pub fn b() -> i32 {\n    42\n}\n");
    f.git(&["add", "b.rs"]);
    // unstaged modification to a.rs
    f.write("a.rs", "fn main() {\n    one();\n    two();\n}\n");
    // staged rename + edit
    f.git(&["mv", "notes.md", "docs.md"]);
    f.write(
        "docs.md",
        "line one\nline two CHANGED\nline three\nline four\nline five\n",
    );
    f.git(&["add", "docs.md"]);
    // untracked file
    f.write("new.rs", "let x = 1;\n");
    f
}

#[test]
fn working_tree_includes_untracked_and_rename() {
    let f = populated();
    let cs = git::load(
        f.path(),
        &LoadRequest::WorkingTree {
            include_untracked: true,
            base: None,
        },
    )
    .unwrap();

    let by_path = |p: &str| cs.files.iter().find(|x| x.path == p);

    let a = by_path("a.rs").expect("a.rs present");
    assert_eq!(a.status, FileStatus::Modified);
    assert_eq!(a.stats.additions, 1);

    let new = by_path("new.rs").expect("untracked present by default");
    assert_eq!(new.status, FileStatus::Untracked);

    let docs = by_path("docs.md").expect("rename target present");
    assert_eq!(docs.status, FileStatus::Renamed);
    assert_eq!(docs.previous_path.as_deref(), Some("notes.md"));
    // body has the one changed line
    assert_eq!(docs.stats.additions, 1);
    assert_eq!(docs.stats.deletions, 1);
}

#[test]
fn exclude_untracked_drops_new_file() {
    let f = populated();
    let cs = git::load(
        f.path(),
        &LoadRequest::WorkingTree {
            include_untracked: false,
            base: None,
        },
    )
    .unwrap();
    assert!(
        cs.files.iter().all(|x| x.path != "new.rs"),
        "untracked should be excluded"
    );
}

#[test]
fn staged_shows_only_index_changes() {
    let f = populated();
    let cs = git::load(f.path(), &LoadRequest::Staged).unwrap();
    // staged: b.rs modification and the docs.md rename; NOT a.rs (unstaged) or new.rs
    assert!(cs
        .files
        .iter()
        .any(|x| x.path == "b.rs" && x.status == FileStatus::Modified));
    assert!(cs
        .files
        .iter()
        .any(|x| x.path == "docs.md" && x.status == FileStatus::Renamed));
    assert!(cs.files.iter().all(|x| x.path != "a.rs"));
    assert!(cs.files.iter().all(|x| x.path != "new.rs"));
}

#[test]
fn path_filter_scopes_to_subtree() {
    let f = populated();
    let mut cs = git::load(
        f.path(),
        &LoadRequest::WorkingTree {
            include_untracked: true,
            base: None,
        },
    )
    .unwrap();
    git::apply_path_filter(&mut cs, &["docs.md".to_string()]);
    assert_eq!(cs.files.len(), 1);
    assert_eq!(cs.files[0].path, "docs.md");
    // rename matches on its previous path too
    let mut cs2 = git::load(
        f.path(),
        &LoadRequest::WorkingTree {
            include_untracked: true,
            base: None,
        },
    )
    .unwrap();
    git::apply_path_filter(&mut cs2, &["notes.md".to_string()]);
    assert!(cs2.files.iter().any(|x| x.path == "docs.md"));
}

#[test]
fn working_tree_from_ref_includes_committed_and_uncommitted() {
    // `diff --from <ref>` compares the working tree (with uncommitted edits)
    // against an arbitrary ref, not HEAD — so it must show both the commits made
    // since the ref and the uncommitted working-tree changes.
    let f = GitFixture::new();
    f.write("a.rs", "base\n");
    f.write("keep.rs", "keep\n");
    f.commit_all("base");
    let base = f.git(&["rev-parse", "HEAD"]).trim().to_string();

    // A commit after the base: modify a.rs, add new.rs.
    f.write("a.rs", "feature\n");
    f.write("new.rs", "added\n");
    f.commit_all("feature work");

    // An uncommitted working-tree edit.
    f.write("keep.rs", "keep edited\n");

    let cs = git::load(
        f.path(),
        &LoadRequest::WorkingTree {
            include_untracked: true,
            base: Some(base.clone()),
        },
    )
    .unwrap();
    let by = |p: &str| cs.files.iter().find(|x| x.path == p);

    // Committed modification relative to the base ref.
    let a = by("a.rs").expect("a.rs vs base");
    assert_eq!(a.status, FileStatus::Modified);
    assert_eq!((a.stats.additions, a.stats.deletions), (1, 1));
    // Committed addition relative to the base ref (tracked, not untracked).
    assert_eq!(
        by("new.rs").expect("new.rs vs base").status,
        FileStatus::Added
    );
    // Uncommitted working-tree modification.
    assert_eq!(
        by("keep.rs").expect("keep.rs vs base").status,
        FileStatus::Modified
    );

    assert_eq!(cs.files.len(), 3, "exactly the three differences vs base");

    // Sanity: against HEAD (default) only the uncommitted edit shows.
    let head = git::load(
        f.path(),
        &LoadRequest::WorkingTree {
            include_untracked: true,
            base: None,
        },
    )
    .unwrap();
    assert_eq!(head.files.len(), 1);
    assert_eq!(head.files[0].path, "keep.rs");
}

#[test]
fn show_detects_a_rename_across_commits() {
    // A commit that renames a file: the tree-to-tree Show diff detects it as a
    // rename (the `Change::Rewrite` arm), carrying the source as `previous_path`.
    let f = GitFixture::new();
    // Enough content that rename detection is confident.
    let body = "line a\nline b\nline c\nline d\nline e\nline f\nline g\nline h\n";
    f.write("old_name.txt", body);
    f.commit_all("c1");
    f.git(&["mv", "old_name.txt", "new_name.txt"]);
    f.commit_all("c2 rename");

    let cs = git::load(f.path(), &LoadRequest::Show { rev: "HEAD".into() }).unwrap();
    let renamed = cs
        .files
        .iter()
        .find(|x| x.path == "new_name.txt")
        .expect("renamed file present in the Show");
    assert_eq!(renamed.status, FileStatus::Renamed);
    assert_eq!(renamed.previous_path.as_deref(), Some("old_name.txt"));
}

#[test]
fn show_head_matches_committed_changes() {
    let f = GitFixture::new();
    f.write("x.rs", "a\nb\n");
    f.commit_all("c1");
    f.write("x.rs", "a\nB\nc\n");
    f.commit_all("c2");

    let cs = git::load(f.path(), &LoadRequest::Show { rev: "HEAD".into() }).unwrap();
    let x = cs
        .files
        .iter()
        .find(|f| f.path == "x.rs")
        .expect("x.rs changed");
    assert_eq!(x.status, FileStatus::Modified);
    assert_eq!(x.hunks.len(), 1);
    assert_eq!(x.hunks[0].header(), "@@ -1,2 +1,3 @@");
}
