//! Shared `#[cfg(test)]` helpers: the git-scratch-repo scaffolding and the blame
//! pump, kept in one place so an environment-driven fixture fix (a git config,
//! an `init` flag) lands once rather than in every test module's private copy.

#![cfg(test)]

use std::path::Path;

use tempfile::TempDir;

/// Run a git command in `dir`, asserting it succeeds (stderr on failure).
pub(crate) fn run_git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A fresh, empty git repository with a review-friendly identity and gpg signing
/// off — the common prelude for every scratch-repo fixture.
pub(crate) fn scratch_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    run_git(dir.path(), &["init", "-q"]);
    run_git(dir.path(), &["config", "user.email", "t@t.t"]);
    run_git(dir.path(), &["config", "user.name", "t"]);
    run_git(dir.path(), &["config", "commit.gpgsign", "false"]);
    dir
}

/// A throwaway repo with two commits, for history/range tests that must not
/// depend on the crate repo's own (squashable) commit count.
pub(crate) fn multi_commit_repo() -> TempDir {
    let dir = scratch_repo();
    std::fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    run_git(dir.path(), &["add", "-A"]);
    run_git(dir.path(), &["commit", "-qm", "first"]);
    std::fs::write(dir.path().join("a.txt"), "two\n").unwrap();
    run_git(dir.path(), &["add", "-A"]);
    run_git(dir.path(), &["commit", "-qm", "second"]);
    dir
}
