//! Program-boundary integration tests: drive the real `rediff` binary as a
//! subprocess so the non-interactive entry points (`main`'s pipe path, the
//! `pager`/`external` filters, and `Config::load`) execute end to end.
//!
//! cargo-llvm-cov instruments subprocesses of the test binary too: the
//! `CARGO_BIN_EXE_rediff` child inherits `LLVM_PROFILE_FILE`, so the coverage of
//! these otherwise terminal-bound functions is captured here.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::GitFixture;

const MODIFY_DIFF: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1234567..89abcde 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 fn main() {
-    let x = 1;
+    let x = 2;
 }
";

/// A temp dir holding `rediff/config.toml` so the child's `Config::load` runs
/// its read + TOML-parse path (rather than the absent-file default).
fn config_home(theme: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("config tempdir");
    let cfg_dir = dir.path().join("rediff");
    std::fs::create_dir_all(&cfg_dir).expect("mk config dir");
    std::fs::write(
        cfg_dir.join("config.toml"),
        format!("theme = \"{theme}\"\nmode = \"stack\"\n"),
    )
    .expect("write config");
    dir
}

/// Run the binary with the given args, a pinned `XDG_CONFIG_HOME`, optional
/// stdin, and a piped (non-TTY) stdout. Returns (stdout, success).
fn run(args: &[&str], xdg: &std::path::Path, stdin: Option<&str>) -> (String, bool) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rediff"));
    cmd.args(args)
        .env("XDG_CONFIG_HOME", xdg)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn rediff");
    if let Some(s) = stdin {
        child
            .stdin
            .take()
            .expect("child stdin")
            .write_all(s.as_bytes())
            .expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait rediff");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.success(),
    )
}

#[test]
fn pager_reads_stdin_and_writes_ansi() {
    let cfg = config_home("dark");
    let (stdout, ok) = run(&["pager"], cfg.path(), Some(MODIFY_DIFF));
    assert!(ok, "pager exits cleanly");
    assert!(!stdout.is_empty(), "pager produced output");
    assert!(stdout.contains("\x1b[38;2;"), "pager emits truecolor ANSI");
    assert!(
        stdout.contains("src/lib.rs"),
        "pager rendered the file header"
    );
}

#[test]
fn pager_with_explicit_theme_flag() {
    // Exercises filter_theme's explicit-flag branch (flag wins over config).
    let cfg = config_home("dark");
    let (stdout, ok) = run(
        &["pager", "--theme", "light"],
        cfg.path(),
        Some(MODIFY_DIFF),
    );
    assert!(ok, "pager --theme exits cleanly");
    assert!(stdout.contains("\x1b[38;2;"), "themed ANSI emitted");
}

#[test]
fn diff_piped_prints_unified_text() {
    // stdout is a pipe (not a TTY) → main takes the synchronous git::load +
    // render::to_unified_string path instead of launching the TUI.
    let f = GitFixture::new();
    f.write("a.rs", "fn main() {\n    one();\n}\n");
    f.commit_all("init");
    f.write("a.rs", "fn main() {\n    one();\n    two();\n}\n");

    let cfg = config_home("dark");
    let repo = f.path().to_str().expect("utf8 repo path");
    let (stdout, ok) = run(&["diff", "-C", repo], cfg.path(), None);
    assert!(ok, "diff exits cleanly");
    assert!(stdout.contains("a.rs"), "unified diff names the file");
    assert!(
        stdout.contains("two()"),
        "unified diff shows the added line"
    );
}

#[test]
fn external_renders_two_files() {
    // GIT_EXTERNAL_DIFF per-file renderer: path old-file old-hex old-mode
    // new-file new-hex new-mode.
    let dir = tempfile::tempdir().expect("tempdir");
    let old = dir.path().join("old.rs");
    let new = dir.path().join("new.rs");
    std::fs::write(&old, "fn main() {\n    let x = 1;\n}\n").expect("write old");
    std::fs::write(&new, "fn main() {\n    let x = 2;\n}\n").expect("write new");

    let cfg = config_home("dark");
    let (stdout, ok) = run(
        &[
            "external",
            "src/lib.rs",
            old.to_str().unwrap(),
            "oldhex",
            "100644",
            new.to_str().unwrap(),
            "newhex",
            "100644",
        ],
        cfg.path(),
        None,
    );
    assert!(ok, "external exits cleanly");
    assert!(stdout.contains("src/lib.rs"), "external names the file");
    assert!(stdout.contains("\x1b[38;2;"), "external emits ANSI");
}
