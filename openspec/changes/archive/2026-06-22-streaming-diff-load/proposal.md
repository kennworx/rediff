## Why

rsdiff loads the entire changeset synchronously before the TUI appears — `git::load` reads every blob and runs the diff for every file on the main thread. For a big change (a branch diff, a large PR, a `diff --from <branch>`) that's seconds of a frozen terminal showing nothing, with no way to bail out. The same freeze hits in-session when picking a large commit in the `c` picker. The file *list* is cheap to produce; only the per-file diff is expensive. We can show the file list instantly and stream the diffs in.

## What Changes

- **Instant file list** — enumerate the changed files (paths, status, renames) synchronously and show the sidebar immediately. This is cheap: gix `status` / `diff_tree_to_tree` give it without reading blob contents.
- **Background, streaming diff** — read blobs + compute hunks per file off the UI thread, populating each file's stats and body as it completes. The sidebar `+/−` stats and the diff pane fill in progressively; nothing blocks.
- **Progress in the diff pane (no popup)** — while files are still computing, the diff pane shows progress (e.g. "diffing 128 / 350") instead of a modal overlay, so the user sees the file list, can navigate, and decides to wait or quit. **Esc/q** works throughout: quit at startup, return to the previous view for a mid-session switch.
- **Parallel diffing** — fan the per-file `compute_hunks` across cores so large changesets finish several× faster, preserving stable file order.
- **No chrome for fast loads** — a small threshold (~80 ms) before any progress is shown, so small changes stay instant and popup/indicator-free.
- **Peek works during load** — the single-file peek (preview/diff) sources its file's content on demand so it works on any file the moment the list appears, even before that file's bulk diff has run. Preview is a single blob read; a single-file diff is one file's work.
- **All load sites** — startup, the `c` commit picker, and `diff --from <branch>` all use the same async loader, so none of them freeze.

## Capabilities

### New Capabilities
- `streaming-diff-load`: the two-stage load (instant file list → background streaming diff), parallel per-file diffing with stable ordering, cancellation (esc/q), the ~80 ms progress threshold, in-diff-pane progress, and the shared loader used by every load site.

### Modified Capabilities
- `changeset-loading`: split loading into a fast enumeration (paths/status, no contents) and a per-file diff step, so files can be listed before they are diffed; add a streaming/progress + cancel interface.
- `review-stream`: render not-yet-diffed files (a stub row + `+? −?` placeholder stats) and show load progress in the diff pane, replacing them as each file's diff lands.
- `file-peek`: source the peeked file's content directly from git (by path + the view's base/new refs) rather than the changeset's cached text, so the peek works on stub files during streaming.

## Impact

- **Model** (`src/model.rs`): a file gains a "not yet diffed" state — either `hunks: Option<…>`/a `stub` flag, or a separate `FileStub` promoted to `DiffFile`. Sidebar/plan must tolerate undiffed files.
- **Git** (`src/git.rs`): split each loader (`working_tree`/`staged`/`show`/`tree_to_tree`/`review_range`) into enumerate (cheap, paths+status) + per-file diff; a parallel driver with an atomic progress counter + cancel flag; the peek's single-file content fetch (carry the view's base ref for `diff --from`).
- **TUI** (`src/tui/`): a `Loader` on `App` (worker handle, progress receiver, cancel flag, done count); a "loading" state where the sidebar is live and the diff pane shows progress; the event loop drains progress + installs files as they arrive and rebuilds the plan incrementally; esc/q semantics (quit vs back); the `c` picker and `--from` switches route through the loader.
- **Dependencies**: parallelism via a small std thread pool (like the highlight worker) — no new crate required; `rayon` optional if simpler.
- **Risk**: incremental plan rebuilds as files stream in must stay cheap (debounce/batch) to avoid churn; cancellation must restore the terminal cleanly.
