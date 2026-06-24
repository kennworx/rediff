## Context

`main.rs` calls `git::load(repo, req)` synchronously, then hands the finished `Changeset` to `tui::run`. `git::load` enumerates changed files and, for each, reads both blobs and runs `compute_hunks` (imara Histogram) — building `DiffFile`s with full `hunks`. The TUI's `Plan`/`SplitPlan`, sidebar, and the highlight worker all assume each `DiffFile` already has its `hunks`. The same synchronous `git::load` runs on the UI thread inside `App::open_commit` (the `c` picker) and behind `diff --from <branch>`.

Cost analysis: enumerating the file list (paths, status, renames) is cheap — gix `status` and `diff_tree_to_tree` provide it without reading blob contents. The expensive part is per file: reading blobs + the diff algorithm (which also yields the `+N −M` stats) + hunk extraction. So stats are *not* free (they require the diff), but the file list is.

## Goals / Non-Goals

**Goals:**
- Show the changed-file list (sidebar) instantly; stream diffs in behind it.
- Diff pane shows progress (no modal popup); esc/q quits (startup) or returns to the previous view (mid-session switch).
- Parallelize per-file diffing, preserving stable file order.
- Small changes show no progress chrome (threshold).
- The single-file peek works during streaming (loads its own file).
- One async loader shared by startup, the `c` picker, and `diff --from`.

**Non-Goals:**
- True lazy "only diff what you view" (we eagerly diff all files in the background; lazy is a possible later step).
- Changing the diff algorithm or its output.
- Background-loading the non-TUI text dump (it stays synchronous).

## Decisions

### Decision 1: Two-stage load — enumerate (sync) then diff (async)
Split each loader into:
1. **`enumerate`** → `Vec<FileStub>` (path, status, previous_path, and the blob handles/oids or side sources needed to diff later). Cheap; runs synchronously so the sidebar is immediate.
2. **`diff_file(stub)`** → a populated `DiffFile` (hunks + stats). Expensive; runs on the worker pool.

A file is therefore in one of two states. Represented as `DiffFile.hunks: Option<Vec<Hunk>>` with `stats: Option<Stats>` (or a `diffed: bool` + empty placeholders). The sidebar/plan render the undiffed state.

**Why a stub stage:** stats need the diff, so we can't show a "complete" list instantly — but we *can* show paths+status instantly and fill the rest in. **Alternative:** keep `DiffFile` whole and just background the entire `Changeset`, installing it at the end (no streaming) — rejected; that delays the file list and provides no per-file value.

### Decision 2: A `Loader` worker, streaming results to the UI
`App` holds `Option<Loader>`:

```
Loader {
    total: usize,                       // file count (known after enumerate)
    done: usize,                        // completed diffs
    rx: Receiver<(usize, DiffFile)>,    // index + populated file, streamed
    cancel: Arc<AtomicBool>,
    _pool: …,                           // worker threads
}
```

The enumerate result seeds the `Changeset` with stubs immediately. The worker pool diffs files and sends `(index, DiffFile)` back; the event loop drains them, replaces stub→diffed at `index`, and rebuilds the plan (batched). When `done == total`, drop the loader.

### Decision 3: Parallel diffing with stable order
Files are diffed concurrently (a pool sized `min(cores, …)` like `HlService`), but each result carries its **original index**, so the `Changeset.files` order never changes — only their contents get filled. An `AtomicUsize` (or the drained count) drives the progress number.

**Why index-tagged results:** keeps display order deterministic regardless of completion order. **Alternative:** `rayon` parallel map collecting in order — simpler but adds a dependency and a barrier (no streaming); rejected in favor of the existing mpsc+pool pattern.

### Decision 4: Progress in the diff pane, threshold-gated
While `Loader` is active and the current file (or the whole set) isn't diffed, the **diff pane** renders a progress line ("diffing 128 / 350 · esc/q to cancel") instead of a popup. The sidebar stays fully live. Progress chrome only appears after the load has run longer than ~80 ms, so fast changes never flash it. A file that is already diffed renders normally even while others stream.

### Decision 5: Cancellation semantics
Esc/q while loading sets `cancel`. At **startup** that means quit (restore terminal, exit). For a **mid-session switch** (the `c` picker / `--from` pushed a loading view) it means abandon the load and return to the previous view. The worker pool checks `cancel` between files and stops; partial results are discarded for a cancelled switch.

### Decision 6: Peek sources its own single file
The peek stops reading `old_text`/`new_text` from the (possibly stub) `DiffFile` and instead loads the selected file's content directly from git: preview = the file at the view's new side (one blob/worktree read); diff = `compute_hunks` over the view's `(base, new)` blobs for that one path. Cheap (one file), and independent of the bulk load's progress. The view must expose its **base ref** for the `=` diff — store it on the view (the `diff --from <branch>` base currently lives only in `LoadRequest`).

### Decision 7: Shared loader at every load site
`tui::run` starts the loader for the launch request; `open_commit` and the `--from` switch push a loading view and start a loader instead of calling `git::load` inline. One code path, one progress/cancel behavior everywhere.

## Risks / Trade-offs

- **Plan churn** as files stream in → batch installs (drain all ready results per tick, rebuild once) and/or rebuild lazily; avoid a full re-plan per file.
- **Undiffed-file rendering** complicates `Plan`/sidebar/`change_starts` (must tolerate `Option` hunks) → keep the stub render trivial (a single placeholder row) and treat stubs as zero-hunk in navigation until filled.
- **Cancellation + terminal restore** must be airtight on every exit path (panic, error, cancel) → centralize teardown.
- **Worker lifetime** vs a cancelled/closed view → the pool must stop and join without blocking the UI; drop the `Loader` to signal shutdown.
- **Stats placeholder** (`+? −?`) churn in the sidebar → render a stable placeholder, swap to real numbers on completion.

## Open Questions

- File-state representation: `Option<hunks>` on `DiffFile` vs a separate `FileStub`→`DiffFile` promotion. The first is less churn; the second is cleaner typing.
- Does the diff pane show *global* progress ("128/350") or only gate the **current** file's body (render its diff the instant that one file is done, progress only when you're on an undiffed file)? The latter feels more responsive.
- Pool sizing and whether to prioritize the files currently on screen (diff visible files first, then the rest) — a cheap scheduling win.
