## Why

In lazygit, the inline diff panel is driven by an external tool. Today that tool is difftastic (`externalDiffCommand: difft`), whose coloring is sparse and — being an `externalDiffCommand` — is **display-only**: the moment you focus a file to stage lines/hunks, lazygit reverts to plain `git diff --no-ext-diff`, and difft's render disappears. We want rediff's own theme + tree-sitter coloring inline, while keeping line/hunk staging intact. The only lazygit integration point that preserves staging is a **pager** (it post-processes git's real patch, so the stageable unified diff still exists underneath). rediff is already a line-diff renderer (`imara-diff` + tree-sitter); it just can't yet consume a patch from stdin.

## What Changes

- Add a non-interactive **`rediff pager`** subcommand: reads a unified diff from **stdin**, renders it with rediff's existing tree-sitter/syntect highlighting and active theme, and writes ANSI to **stdout**. It does not launch the TUI.
- Color is **forced on** when stdout is not a TTY (lazygit/git capture via a pipe), so the render survives piping.
- Add `diffy` (0.5, `patch_set` + `ParseOptions::gitdiff()`) as the unified-diff parser — rediff currently computes diffs, it has no patch parser.
- Render git-aware file metadata the parser surfaces: file operation (create / delete / modify / rename / copy), mode changes, and binary files (shown as a notice rather than choking).
- Reuse the existing highlight primitives (`highlight::{Span, Paint, FileHighlight, Highlight}`) and theme resolution; the new code is a stdin→model adapter plus an ANSI line renderer in `render.rs` (alongside the existing plain `to_unified_string`).
- (Phase 2, shipped as **`rediff external`**) a `GIT_EXTERNAL_DIFF` per-file renderer (full-file context, best colors) for terminal `git diff`/`show`/`log -p`, and an alternative lazygit `externalDiffCommand`. Note it does **not** add untracked files to lazygit's combined view — no diff tool can, since that view is `git diff` (see design.md Outcome).
- Document the lazygit config swap: replace `externalDiffCommand: difft` with `pager: rediff pager`, which restores line/hunk staging.

## Capabilities

### New Capabilities
- `diff-pager`: a non-interactive mode that consumes a unified diff on stdin and emits a syntax-highlighted, themed, ANSI-colored diff on stdout, suitable as a git/lazygit pager that preserves staging.

### Modified Capabilities
<!-- None. The pager reuses the existing syntax-highlighting and theme-selection capabilities without changing their requirements. -->

## Impact

- **New dependency**: `diffy = "0.5"` (binary delta decoding is feature-gated and not required — only binary *detection* is needed).
- **`src/cli.rs`**: new `Pager` subcommand (and optional `Print`); these resolve to a headless render path instead of `tui::run`.
- **`src/main.rs`**: route `pager`/`print` to the new renderer before the `is_terminal()` TUI branch.
- **`src/render.rs`**: new ANSI renderer for highlighted hunk lines (the existing `to_unified_string` stays for the current plain-pipe fallback).
- **New module** (e.g. `src/pager.rs`): diffy `PatchSet` → reconstructed old/new line sequences → highlighter → ANSI.
- **Highlighting quality caveat**: a pager only sees hunk fragments (a few context lines), not whole files, so tree-sitter has less context than the TUI; highlighting is best-effort at hunk edges. (`print` mode, which receives whole files, does not have this limitation.)
- **User config** (`dotfiles/dot_config/lazygit/config.yml`): swap difft `externalDiffCommand` for `rediff pager` pager; not part of this repo's code but the change's reason for existing.
- No change to the interactive TUI, existing subcommands, or their specs.
