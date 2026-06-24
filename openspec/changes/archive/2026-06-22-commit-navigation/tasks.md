## 1. View-stack refactor (foundation)

- [x] 1.1 Introduce `ViewEntry { kind, source_label, cs: Rc<Changeset>, scroll, selected, review: Option<Vec<bool>> }` and a `ViewKind { Local, Staged, Commit(id), Range }` in the tui module
- [x] 1.2 Change `App` to own `views: Vec<ViewEntry>` + `cursor: usize`; drop the `App<'a>` lifetime and the borrowed `cs`/global `viewed` fields
- [x] 1.3 Add accessors (`cur()`, `cur_cs()`, `cur_review()`) and a `rebuild_plan()` that builds `plan`/`split` from `cur_cs()` and the current view's review vector (all-false when browsing)
- [x] 1.4 Update `App::new`/`with_mode`/`with_options` construction to seed a single home `ViewEntry`; fix the ~30 test call sites to the owned-changeset shape
- [x] 1.5 Thread a repo handle (or repo dir) into `tui::run()` so views can be loaded live; update `main.rs` to pass it
- [x] 1.6 Verify existing behavior is unchanged with one seeded view (all current tests green)

## 2. Commit enumeration & file-scoped history (git layer)

- [x] 2.1 Add `CommitInfo { short_sha, summary, author, time }` to the model
- [x] 2.2 Implement `enumerate_commits(repo, tip, cap=200) -> Vec<CommitInfo>` via gix rev-walk, newest first, with a truncation flag
- [x] 2.3 Implement `commits_touching(repo, path, &[CommitInfo]) -> Vec<CommitInfo>` comparing the path's blob oid in each commit's tree vs its parent's tree
- [x] 2.4 Unit-test enumeration order/cap and file-scoped filtering against a small fixture repo

## 3. Review command & range net diff (CLI + git)

- [x] 3.1 Add `Command::Review { sha: Option<String>, from: Option<String>, repo, mode, theme, targets }` to the CLI and resolve it
- [x] 3.2 Add a `LoadRequest::Review { target, from: Option<String> }` and load it: single commit (target vs parent) when `from` is None
- [x] 3.3 Implement range net diff: merge-base of `from` and `target`, then `tree_to_tree(merge_base_tree, target_tree)`; fall back to two-dot tree diff when no merge-base exists
- [x] 3.4 Set the view's `review = Some(vec![false; files])` and a green source label for review/commit; unit-test single vs range loading

## 4. Commit picker overlay & smart filter

- [x] 4.1 Generalize the palette into a kind-tagged overlay (file-jump vs commit-pick), or add a parallel `CommitPicker` reusing input/move/pick/confirm + number shortcuts
- [x] 4.2 Implement the smart filter dispatch: hex prefix → SHA match; known-path prefix → file-scoped list; else fuzzy `score` over summary; show the active mode in the popup
- [x] 4.3 Bind `c` to open the picker over commits from HEAD (cap 200)
- [x] 4.4 When the current view is a range review, exclude the range's own commits (`base..target` set) from the `c` and `F` lists; no exclusion otherwise
- [x] 4.5 On confirm: load `show(commit)`, push a new browse `ViewEntry` (truncating forward history), switch the view
- [x] 4.6 Tests: filter dispatch picks the right mode; picking pushes a view and switches; range commits are excluded during a range review

## 5. File-scoped log (`F`) & path detection

- [x] 5.1 Bind `F` (from any focus) to open the picker scoped to the selected file via `commits_touching`
- [x] 5.2 On confirm from a file-scoped list, switch to the commit and set `selected` to that file when the commit changed it
- [x] 5.3 Implement the "known repo path" check used by both `F` and the `c` path-filter (prefix-match against current changeset + HEAD tree)
- [x] 5.4 Tests: `F` lists only touching commits; landing selection is on the file

## 6. View-history navigation (`{`/`}`/`C`/`R`)

- [x] 6.1 Bind `{`/`}` (matched as `Char('{')`/`Char('}')`) to cursor back/forward with per-view scroll/selection restore
- [x] 6.2 Bind `C` (`Char('C')`) to jump to the home view when it is local/staged/review; inert otherwise
- [x] 6.3 Bind `R` (`Char('R')`) to promote the current browse view into a review session (attach a fresh `viewed`, flip the flag)
- [x] 6.4 Tests: back/forward restores position; new view truncates forward history; `C` inert when launched on a commit; `R` enables tracking

## 7. Per-view review gating (`v`/`u`/`✓`)

- [x] 7.1 Gate `toggle_viewed`, `next_unviewed`, the reviewed count, and collapse-on-viewed on `cur_review().is_some()`
- [x] 7.2 Build the plan with the current view's review vector (all-false when browsing, so no collapse)
- [x] 7.3 Tests: marking inert while browsing; review progress persists across a browse round-trip

## 8. Highlight reset on switch

- [x] 8.1 Add an `epoch: u32` to `HlService`; tag requests; discard drained results whose epoch is stale
- [x] 8.2 On every view switch, call `hl.clear()` and bump the epoch; re-request visible files
- [x] 8.3 Test: a late in-flight result from the previous view is discarded after a switch

## 9. Source color coding & status

- [x] 9.1 Add a blue/green source-accent pair to `Theme` for both dark and light (reuse existing colors where possible)
- [x] 9.2 Tint the status-line source label and sidebar file markers by the current view's source
- [x] 9.3 Show a review session's `✓ n/m` (and a truncation hint for the commit list) in the status; grey unavailable `C`/`v` hints
- [x] 9.4 Render test: blue for local, green for a commit view

## 10. Wrap-up

- [x] 10.1 Update the in-TUI help/status hints to list `c`/`F`/`C`/`R`/`{`/`}`
- [x] 10.2 `cargo test`, `cargo clippy` clean; manual dogfood of the picker, file history, back/forward, and a range review
