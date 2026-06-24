## Why

When reviewing, the 3-line hunk context often isn't enough — you want to see the whole file, or how a historical version of a file relates to what you're reviewing now. Today rsdiff can only show the fixed change for the current view. This adds a focused single-file "peek" overlay that answers two recurring questions without leaving your place: *"show me this whole file"* and *"how does this file's version here differ from the top of what I'm reviewing?"* — with adjustable diff context. It's a context/overview helper, not a second review surface.

## What Changes

- **Single-file peek overlay** — a modal, full-area, scrollable, syntax-highlighted view of **one** file. It is ephemeral (not part of the `<`/`>` view history) and `Esc` returns you exactly where you were.
- **Two modes, toggled with `Tab`** — **content** (the full file, no diff markers) and **diff** (a unified diff for that file). No viewed-tracking in either.
- **Adjustable diff context** — `=`/`+` expand and `-`/`_` compact the surrounding context lines in diff mode (down to a minimal hunk view, up to the whole file).
- **Two open keys from the selected file**, which differ only in what the diff compares — both diffs end at `TOP`, the newest side of the current review context (working copy for a working-tree review, the target commit for a `review base..target`, the commit itself for a single commit):
  - **`p` (history)** — preview the file *at the commit you're viewing / drilled into*; its diff is `that commit → TOP` ("what changed since then"). Opens in content mode.
  - **`=` / `+` (review)** — preview the file *at `TOP`*; its diff is the view's own change (`base → TOP`). Opens directly in diff mode with context expanded.
- **Source color** — the overlay inherits the origin's accent: blue when opened from a local/staged view, magenta from a commit/range view.

## Capabilities

### New Capabilities
- `file-peek`: the single-file overlay — its content/diff modes, the `p` (history) and `=` (review) open keys, the `TOP`-anchored diff semantics, `Tab` toggling, `=`/`-` context-level control, source coloring, and that it is modal (no view-history entry) with no viewed-tracking.

### Modified Capabilities
- `changeset-loading`: add the loads the peek needs — a single file's full content at a given revision (for content mode), a single-file unified diff at an arbitrary context level, and resolving `TOP` (the newest side of the current review context) so the history diff compares against it rather than always against the working copy.

## Impact

- **TUI** (`src/tui/`): a new modal peek state on `App` holding one file's content/diff, its own scroll, a mode (content/diff), and a context level; a renderer for it that reuses the existing row/highlight/render functions over a one-file changeset; key handling for `p`, `=`/`+`, `-`/`_`, `Tab`, scroll, and `Esc` (the peek captures input while open); a dedicated highlight slot for the peeked file. Source color reuses the existing local/commit accents.
- **Git/diff** (`src/git.rs`, `src/diff.rs`): a helper to load a file's blob text at a rev and read its working-copy text; reuse of `compute_hunks_with_context` for the context level; an all-context builder for content mode; resolution of the current view's `TOP`.
- **No new dependencies.** Speed budget unaffected — a single file's content/diff is trivial, highlighting stays async.
