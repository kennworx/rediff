## Context

rsdiff is a greenfield Rust TUI that re-implements hunk's review-first git-diff viewer. hunk
(TypeScript/Bun/OpenTUI/Pierre) is usable but slow on every axis a reviewer feels: ~68 ms per
keypress, ~33 ms first frame, ~76 ms highlight, ~40 ms Bun cold start. The product thesis has
two halves — **machine-fast** (latency you measure) and **human-fast** (keystrokes to the
change you care about) — and rsdiff must win both.

All load-bearing technical bets were validated by spikes before this design (code in
`tmp/spikes/`, numbers in `tmp/spikes/FINDINGS.md`):
- gix 0.83 `repo.status()` + `diff_tree_to_tree` classify every change type; with `imara-diff`
  on blob/worktree pairs the whole git→hunks path runs in <5 ms and produces git-identical diffs.
- tree-sitter highlights a full 1868-line TSX file at 96.7% coverage in ~7 ms; syntect on the
  same file takes 128 ms (~18×). Cold grammar/theme load is ~1 ms for both.
- Renames decode cleanly from `ChangeRef::Rewrite` and match `git diff -M`.

## Goals / Non-Goals

**Goals:**
- Sub-2 ms keypress navigation and first frame well under hunk's numbers.
- One normalized changeset model that drives the sidebar, the review stream, navigation,
  scrolling, and note placement from a single planning layer (no parallel implementations).
- Highlighting that never blocks input.
- Usability at least at hunk parity, plus fuzzy file jump and viewed tracking.

**Non-Goals:**
- Pager mode, difftool, raw file↔file diffing, Jujutsu/Sapling, and the agent-daemon / session
  system. These are deliberately excluded from this change.
- Custom-theme TOML inheritance (hunk has it); MVP ships built-in dark + light only.
- Structural/semantic diffing (difftastic-style). rsdiff is line-based via imara-diff.

## Decisions

### Git access: gix 0.83 + imara-diff (not libgit2, not shelling out)
gix is pure Rust, no git binary dependency, and gives us blob ids directly so we run our own
diff. We turn blob/worktree pairs into hunks with `imara-diff` (Histogram algorithm), yielding
one internal hunk model for every input source. **Alternatives:** shelling out to `git` (keeps
subprocess overhead, the thing we're removing); libgit2/git2 (C dependency, heavier API).
**Gotcha captured by spike:** gix 0.84.0 is unresolvable on crates.io (wants an unpublished
`gix-credentials`); pin **0.83**, and keep gix default features on because
`default-features=false` drops the sha1 hash backend (compile error).

### Highlighting: tree-sitter primary, syntect fallback, behind one trait
A diff viewer highlights immutable snapshots once and caches — tree-sitter's incremental
edge is moot, but its raw one-shot throughput wins 5–18×, and full-coverage highlighting costs
almost nothing extra (parsing dominates; running more query patterns over the tree is nearly
free). We bundle ~15–20 grammars and assemble each language's query set (base + dialect +
injections). syntect (220 langs via two-face) stays as a breadth fallback behind a single
`Highlighter` trait so unsupported languages still color. **Alternatives:** syntect-only
(128 ms TSX stalls); tree-sitter-only (no coverage for unbundled languages).

### Highlighting runs off the input thread
Highlighting is a pluggable async producer of styled spans: a worker tokenizes a file, the UI
renders plain text for the frame(s) until results land, then swaps in styled spans from a
per-file cache. Scrolling into an un-highlighted file is never a stall. This is what lets the
highlighter choice be an implementation detail rather than a latency risk.

### TUI: ratatui 0.30, immediate-mode + windowing
Navigation is a viewport offset change over ~visible rows, not a React-style tree reconcile —
this is what eliminates hunk's 68 ms keypress. One planning layer derives row geometry; the
sidebar, stream, scrollbar, and hunk navigation all read from it rather than re-deriving.

### One normalized model, sidebar is navigation
`Changeset { files: DiffFile[] }`; `DiffFile { path, hunks, stats, lang, viewed, highlight
cache }`. File order from the loader is authoritative for both sidebar and stream. Selecting a
sidebar file **jumps** to its position in the single continuous stream; it never filters the
stream down to one file. Untracked files are included by default (match hunk, not `git diff`).

## Risks / Trade-offs

- **gix working-tree status is newer code than its plumbing** → spike confirmed it classifies
  staged/unstaged/untracked/rename correctly; if an edge case misbehaves, fall back to
  `git status --porcelain` for the file list only and keep gix for blobs (≈30 lines, not a
  rearchitecture).
- **Per-language tree-sitter query assembly is real work** (TSX needs JS + JSX + TS queries
  stitched) and grammars carry version-compat friction → bound it to ~15–20 curated languages;
  syntect fallback covers the long tail; treat query bundles as data, tested per language.
- **Highlighting heavy files still costs ~7–130 ms of work** even off-thread → cache per file,
  parallelize across files, highlight lazily as files scroll into view; never on the input path.
- **Async highlight + cache adds concurrency complexity** → keep the `Highlighter` trait narrow
  (input: file text + language; output: styled spans) so the worker/cache layer is isolated and
  unit-testable without the TUI.
- **Rename similarity %** — gix doesn't hand back git's exact score in the variant; render a
  "renamed" badge like hunk instead of a percentage (spike showed the body diff is correct).

## Open Questions

- Final bundled-language list for tree-sitter (Rust, TS/TSX, JS/JSX, Python, Go, JSON, etc.).
- Exact key bindings for fuzzy jump and viewed tracking (keep hunk parity where it exists;
  do not reintroduce `j`/`k` hunk nav — `[`/`]` only).
- Whether split view ships in this change or is staged after the single-stream core proves out.
