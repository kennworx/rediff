## 1. Model: undiffed file state

- [x] 1.1 Represent a not-yet-diffed file: `DiffFile.hunks: Option<Vec<Hunk>>` + `stats: Option<Stats>` (or a `diffed` flag with empty placeholders); update constructors
- [x] 1.2 Update everything that reads `hunks`/`stats` to tolerate the undiffed state (default to empty/zero for navigation)
- [x] 1.3 Keep existing tests compiling against the new shape

## 2. Git: split enumerate vs diff

- [x] 2.1 Add `enumerate(repo, req) -> Vec<FileStub>` (path, status, previous_path, side sources) for each source: working tree, staged, show, range, review-range, working-tree-from-ref — no blob reads, no hunks
- [x] 2.2 Add `diff_file(repo, stub) -> DiffFile` that reads the two sides and computes hunks+stats for one file
- [x] 2.3 Verify enumerate is cheap (no contents) and that enumerate+diff_file over all files reproduces today's `load` output (golden test)

## 3. Loader: background, parallel, cancellable

- [x] 3.1 `Loader` type: worker pool (sized like `HlService`), `total`, `done`, `cancel: Arc<AtomicBool>`, `rx: Receiver<(usize, DiffFile)>`
- [x] 3.2 Diff files concurrently, each result tagged with its original index for stable order; bump a progress counter
- [x] 3.3 Check `cancel` between files; stop and join the pool on drop
- [ ] 3.4 Optionally prioritize on-screen files first, then the rest (deferred — workers pull in index order, which front-loads the first screen)
- [x] 3.5 Tests: streaming yields all files in order; cancel stops further work

## 4. App: loading state + streaming install

- [x] 4.1 Seed the changeset with stubs from `enumerate`; start a `Loader`
- [x] 4.2 Event loop drains ready `(index, DiffFile)` results each tick, replaces stubs, rebuilds the plan once per batch (not per file)
- [x] 4.3 Track elapsed time; only show progress chrome after the ~80 ms threshold
- [x] 4.4 Drop the `Loader` when `done == total`
- [x] 4.5 Tests: stubs install, batched plan rebuild, threshold gating

## 5. Rendering: stubs + progress in the diff pane

- [x] 5.1 Sidebar: render stub files with status + placeholder `+? −?` stats; swap to real stats on completion
- [x] 5.2 Diff pane: show progress ("diffing N / M · esc/q") when positioned on undiffed content; render a completed file's diff normally
- [x] 5.3 Plan/`change_starts`/navigation tolerate stub (zero-hunk) files
- [x] 5.4 Render tests: sidebar placeholder, diff-pane progress, completed file renders

## 6. Cancel + input

- [x] 6.1 Esc/q during the launch load → quit (restore terminal, exit)
- [x] 6.2 Esc/q during a mid-session switch load → abandon, return to the previous view
- [x] 6.3 Centralize terminal teardown so every exit path (cancel/error/panic) restores cleanly
- [x] 6.4 Tests: cancel-at-startup quits; cancel-switch restores previous view

## 7. Unify load sites

- [x] 7.1 `tui::run` starts the loader for the launch request (replace the sync `git::load` in `main.rs`)
- [x] 7.2 `open_commit` (the `c` picker) pushes a loading view and starts a loader instead of calling `git::load` inline
- [x] 7.3 `diff --from <ref>` switches route through the loader too
- [x] 7.4 Non-TUI text dump stays synchronous (unchanged)

## 8. Peek during streaming

- [x] 8.1 Carry the view's base ref on the view (so `=` knows the old side for `diff --from`)
- [x] 8.2 Peek sources the selected file's content from git (preview: one blob/worktree read; diff: compute_hunks over the view's base/new for that path), independent of the bulk load
- [x] 8.3 Tests: preview/diff a stub (not-yet-diffed) file works

## 9. Wrap-up

- [x] 9.1 Manual dogfood: a big `diff --from <branch>` and a large commit pick — list instant, progress in the diff pane, esc/q quits/returns, peek works mid-load (dogfooded; fixed: q-freeze under load, sidebar-click offset, scroll-snap during streaming)
- [x] 9.2 `cargo test`, `cargo clippy` clean; confirm small changes show no progress chrome
