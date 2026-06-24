//! Reusable git fixture builder for integration tests.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A throwaway git repository for exercising the loaders.
pub struct GitFixture {
    pub dir: TempDir,
}

impl GitFixture {
    /// Create an empty initialized repo with a deterministic identity.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let f = GitFixture { dir };
        f.git(&["init", "-q"]);
        f.git(&["config", "user.email", "t@t.t"]);
        f.git(&["config", "user.name", "t"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Run a git command in the fixture, asserting success.
    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Write a file (creating parent dirs) relative to the repo root.
    pub fn write(&self, rel: &str, contents: &str) {
        let p: PathBuf = self.path().join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs for test file");
        }
        std::fs::write(p, contents).expect("write test file");
    }

    /// Stage everything and commit with a message.
    pub fn commit_all(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-qm", msg]);
    }
}
