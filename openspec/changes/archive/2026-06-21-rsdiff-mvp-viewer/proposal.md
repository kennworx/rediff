## Why

Reviewing agent-authored git changesets in the terminal today means either plain `git diff`
(no navigation) or hunk — a review-first TUI built on Bun/OpenTUI/Pierre that is genuinely
usable but slow: ~68 ms per keypress for hunk navigation, ~33 ms first frame, ~76 ms syntax
highlight, and ~40 ms Bun cold-start on every invocation. rsdiff re-implements hunk's viewer
in Rust to make the same review workflow feel instant, while keeping usability at least as
good. The core technical bets are already validated by spikes (gix + imara-diff produce
git-identical diffs in <5 ms; tree-sitter highlights a full TSX file in ~7 ms vs syntect's
128 ms; rename decode matches `git diff -M`).

## What Changes

- New `rsdiff` CLI binary with `diff`, `diff --staged`, `show [ref]`, and range commands that
  load git changes into one normalized changeset model (untracked files included by default,
  matching hunk; renames decoded properly).
- A review-first TUI: a single top-to-bottom stream of all changed files with windowed
  rendering, mouse + keyboard scrolling, and a navigation sidebar. Selecting a file in the
  sidebar **jumps** to it within the one continuous stream — it never collapses to a
  single-file view.
- Cross-stream hunk navigation (`[` / `]`), fuzzy file jump (command-palette style), and
  GitHub-PR-style viewed tracking (mark reviewed, jump to next-unviewed, collapse reviewed).
- Responsive `auto` / `split` / `stack` layout modes.
- Async, non-blocking syntax highlighting (tree-sitter primary with a syntect breadth
  fallback behind one `Highlighter` trait), with a per-file highlight cache.
- Theming (at least one dark + one light theme) and a `~/.config/rsdiff/config.toml`.
- **Explicitly OUT of scope** (non-goals for this change): pager mode, difftool, raw
  file↔file diffing, Jujutsu/Sapling support, and hunk's agent-daemon / `session` command
  system.

## Capabilities

### New Capabilities
- `changeset-loading`: Resolve git input (`diff`, `--staged`, `show [ref]`, range) via gix into
  one normalized changeset of files and hunks, including untracked files and decoded renames.
- `review-stream`: Render all changed files as one windowed, scrollable top-to-bottom review
  stream that stays performant on large changesets.
- `navigation`: Sidebar file list with stats and jump-to-file, fuzzy file jump, and `[`/`]`
  hunk navigation across the entire stream.
- `viewed-tracking`: Track per-file/per-hunk reviewed state, jump to next-unviewed, and
  collapse reviewed files in the sidebar.
- `syntax-highlighting`: Asynchronously highlight diff content off the input thread with a
  pluggable highlighter (tree-sitter primary, syntect fallback) and a per-file cache.
- `layout-modes`: Provide `auto`, `split`, and `stack` layouts, with `auto` choosing split on
  wide terminals and stack on narrow ones, and explicit modes overriding the responsive choice.
- `theming-and-config`: Built-in dark/light themes and persisted preferences via a TOML config
  file plus CLI flags.

### Modified Capabilities
<!-- None — this is a greenfield project with no existing specs. -->

## Impact

- New Rust workspace/binary `rsdiff` (currently empty repo, no commits yet).
- Dependencies: `gix = 0.83` (NOT 0.84 — broken on crates.io via an unresolvable
  `gix-credentials` dep; keep gix default features on, since `default-features=false` drops the
  sha1 hash backend), `imara-diff`, `tree-sitter` + per-language grammar crates,
  `syntect` + `two-face`, `ratatui` 0.30, `clap`.
- Per-language tree-sitter query bundles must be assembled (base + dialect + injection queries,
  e.g. TSX = JS `HIGHLIGHT_QUERY` + `JSX_HIGHLIGHT_QUERY` + TS `HIGHLIGHTS_QUERY`).
- Validation spikes live in `tmp/spikes/` with measured results in `tmp/spikes/FINDINGS.md`.
