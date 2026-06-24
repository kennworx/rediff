## 1. Project setup

- [x] 1.1 Initialize the Rust workspace and `rsdiff` binary crate with module skeleton (cli, git, diff, model, highlight, tui, config)
- [x] 1.2 Add dependencies: `gix = "0.83"` (default features on), `imara-diff`, `ratatui` 0.30, `clap`, `syntect`, `two-face`, `tree-sitter` + initial grammar crates; pin versions and document the gix-0.84-is-broken note
- [x] 1.3 Define the core model types: `Changeset`, `DiffFile`, `Hunk`, `Line`, `ViewedState`
- [x] 1.4 Set up unit-test scaffolding and a reusable git fixture builder (port the scratch fixture from `tmp/spikes`)

## 2. Phase 1 — data path (git → hunks, no TUI)

- [x] 2.1 Implement `clap` CLI for `diff`, `diff --staged`, `diff --exclude-untracked`, `show [ref]`, and range
- [x] 2.2 Implement gix working-tree loading (staged + unstaged + untracked) into the model, untracked included by default
- [x] 2.3 Implement gix `show [ref]` and range loading via `diff_tree_to_tree`
- [x] 2.4 Implement blob/worktree → hunks via `imara-diff` (Histogram); produce git-faithful bodies and headers
- [x] 2.5 Decode `ChangeRef::Rewrite` into rename/copy entries (source + destination, "renamed" indication, body)
- [x] 2.6 Add a debug stdout dump of the changeset and tests asserting parity with `git diff` / `git show` on the fixture

## 3. Phase 2 — speed core (ratatui review stream)

- [x] 3.1 Build the app shell and main loop (state, input, render) in ratatui
- [x] 3.2 Implement the single top-to-bottom review stream with a shared row-planning/geometry layer
- [x] 3.3 Implement windowed rendering (viewport + overscan) so large changesets stay responsive
- [x] 3.4 Implement keyboard and mouse-wheel scrolling
- [x] 3.5 Implement the navigation sidebar (file list + add/remove stats) and jump-to-file (jump within the stream, never collapse it)
- [x] 3.6 Implement `[` / `]` hunk navigation across the whole stream (no `j`/`k` hunk binding)
- [x] 3.7 Add a keypress/first-frame latency benchmark and verify it beats hunk's 68 ms / 33 ms targets

## 4. Phase 3 — syntax highlighting (async)

- [x] 4.1 Define the `Highlighter` trait (input: text + language; output: styled spans)
- [x] 4.2 Implement the tree-sitter highlighter and assemble per-language query bundles (base + dialect + injections; TSX = JS HIGHLIGHT_QUERY + JSX_HIGHLIGHT_QUERY + TS HIGHLIGHTS_QUERY)
- [x] 4.3 Implement the syntect fallback (two-face) for languages without a bundled grammar
- [x] 4.4 Implement the off-thread highlight worker with a per-file cache; render plain text until results land
- [x] 4.5 Wire highlighting into the stream renderer and verify input never blocks on highlighting
- [x] 4.6 Add tests/benchmarks for full-coverage highlighting and non-blocking behavior

## 5. Phase 4 — usability

- [x] 5.1 Implement fuzzy file jump (filter by substring, select to jump)
- [x] 5.2 Implement viewed tracking: mark/unmark reviewed, reflected in the sidebar
- [x] 5.3 Implement jump-to-next-unviewed and the all-reviewed terminal state
- [x] 5.4 Implement collapse of reviewed files in the sidebar

## 6. Phase 5 — layout modes

- [x] 6.1 Implement the split (side-by-side) renderer from the same normalized model
- [x] 6.2 Implement stack (unified) and the `auto`/`split`/`stack` mode selection
- [x] 6.3 Implement responsive `auto` selection by terminal width with re-evaluation on resize; explicit modes override

## 7. Phase 6 — polish

- [x] 7.1 Implement built-in dark + light themes and a runtime theme selector
- [x] 7.2 Implement `~/.config/rsdiff/config.toml` loading (theme, mode, line numbers, wrap) with CLI-flag override and safe defaults when absent
- [x] 7.3 Round out mouse support for primary actions (sidebar select, scroll)

## 8. Verification

- [x] 8.1 Run the full latency/highlight benchmark suite and record results against hunk's 0.16.0 numbers
- [x] 8.2 Confirm diff/rename/untracked output parity with git on the fixture and a real repo
- [x] 8.3 Manual TTY smoke run on a real changeset across all three layout modes
