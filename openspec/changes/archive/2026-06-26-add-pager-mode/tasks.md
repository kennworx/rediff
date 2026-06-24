## 1. Dependency & CLI surface

- [x] 1.1 Add `diffy = "0.5"` to `Cargo.toml` (no extra features; binary decoding stays off), verify it builds
- [x] 1.2 Add `Command::Pager` to `src/cli.rs` (no positional targets; accepts `--theme`), with a doc comment describing the stdin→stdout pager
- [x] 1.3 In `src/main.rs`, route `Command::Pager` to the headless renderer **before** the `is_terminal()` TUI branch so it never enters the TUI

## 2. Patch parsing (diffy adapter)

- [x] 2.1 Create `src/pager.rs`; read all of stdin into a buffer
- [x] 2.2 Parse with `diffy::patch_set::PatchSet::parse(input, ParseOptions::gitdiff())`, iterating `FilePatch`es
- [x] 2.3 Map `FilePatch::operation()` (Create/Delete/Modify/Rename/Copy) and `old_mode()/new_mode()` into an internal per-file header model
- [x] 2.4 Branch on `PatchKind`: `Binary` → binary-notice model; `Text(Patch)` → hunks
- [x] 2.5 For each `Hunk`, reconstruct old-side and new-side line sequences from `lines()` (`Line::{Context,Delete,Insert}`), preserving `old_range()/new_range()` and `function_context()`

## 3. Highlighting + ANSI rendering

- [x] 3.1 Resolve the active theme (CLI `--theme` > config) reusing the existing theme resolution from `main.rs`
- [x] 3.2 Run the existing `highlight::Highlight` engine over each file's reconstructed content to get `FileHighlight`/`Span`/`Paint` (one-shot; no `tui/highlight.rs` cache)
- [x] 3.3 ANSI line renderer that writes `Paint`→SGR truecolor with theme-sourced add/del/context backgrounds and the `+`/`-`/space gutter (kept in `src/pager.rs` with the parsing/orchestration rather than `render.rs`, for cohesion)
- [x] 3.4 Force color unconditionally in this path (stdout is a pipe); do not gate on `is_terminal()`
- [x] 3.5 Render file headers (path, rename old→new, status) and the `@@` hunk header (with `function_context`) themed
- [x] 3.6 Render the binary-notice case as a themed line, never raw bytes

## 4. Edge cases & tests

- [x] 4.1 Handle empty stdin → no output (or lone newline), exit 0
- [x] 4.2 Mirror `to_unified_string` handling for no-newline-at-eof and non-UTF8/CRLF; fall back to plain for un-highlightable content
- [x] 4.3 Fixture tests: feed representative `git diff` outputs (modify, add, delete, rename, binary, multi-file) into the pager and assert ANSI output structure (gutter, color presence, file/hunk headers)
- [x] 4.4 Snapshot/golden test that piped output contains ANSI escapes (forced color)
- [x] 4.5 `cargo test` green and `cargo clippy --all-targets` clean

## 5. Integration & docs

- [x] 5.1 Update `dotfiles/dot_config/lazygit/config.yml`: replace the difft `externalDiffCommand` entry with `pager: rediff pager`
- [ ] 5.2 Manually verify in lazygit: inline diff shows rediff colors AND line/hunk staging works again (focus a file, stage a hunk) — requires interactive lazygit
- [x] 5.3 `just install` so the wired binary reflects `rediff pager`
- [x] 5.4 Note the `rediff pager` usage in README/help text

## 6. `rediff external` (GIT_EXTERNAL_DIFF mode)

A `GIT_EXTERNAL_DIFF` driver that diffs whole files (full tree-sitter context).
Explored as a way to show untracked files in lazygit's combined view, but that proved
impossible for any diff tool: lazygit builds the combined view from `git diff`, which
excludes untracked (measured identical for difft, `pager`, and `external`). `external`
still earns its place — best-quality colors for plain-terminal `git diff` and an
alternative `externalDiffCommand`. Per-file hunk staging is unaffected either way
(lazygit renders its own staging view). See design.md Outcome.

- [x] 6.1 Add `Command::External` accepting git's `GIT_EXTERNAL_DIFF` positional args (path, old-file, old-hex, old-mode, new-file, new-hex, new-mode); resolve display path (arg[0], or arg[7]/arg[4] when arg[0] is `/dev/null` for `--no-index` creates)
- [x] 6.2 Read the two whole files, `diff::compute_hunks` → full-file-context highlight (real line numbers index the highlighted lines directly) → reuse the shared ANSI renderer; binary detection via NUL-byte heuristic
- [x] 6.3 Tests for modify/added/deleted(`/dev/null`)/binary/display-path; `cargo test` + `cargo clippy --all-targets` clean
- [x] 6.4 `just install`; lazygit stays on `pager: rediff pager` (external is available but not the default wiring)
- [x] 6.5 Verify `rediff external` renders via `GIT_EXTERNAL_DIFF` in a terminal (tracked, untracked-per-file, binary)
