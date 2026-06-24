## 1. Per-view collapsed state + the selection enum

- [x] 1.1 Add the collapsed-directory set to `ViewState` (e.g. `collapsed: BTreeSet<String>`), so it round-trips through the view history with the rest of per-view state
- [x] 1.2 Introduce `Selection { File(usize), CollapsedFiles(String) }` and replace `ViewState.selected: usize` with it (or keep `selected: usize` + a parallel marker — pick the encoding that reads cleanest); a helper `selected_file() -> Option<usize>` for the common "file or nothing" case
- [x] 1.3 Thread the enum through every reader of `selected`: stream nav, sidebar highlight/hit-test, body scroll-to, review (`toggle_viewed`), peek, jump digits — each acts only on `File` and is inert on `CollapsedFiles`
- [x] 1.4 `toggle_grouping` (`D` → flat) with a `CollapsedFiles` placeholder selected converts the selection to that directory's first file (flat view has no placeholders); the collapsed set is kept and restored when grouped view returns
- [x] 1.5 Tests: file actions (`v`, peek, digit) are inert when a placeholder is selected; `D` from a placeholder selection lands on a file

## 2. Sidebar row model + builder

- [x] 2.1 Add `SidebarRow::CollapsedFiles { dir, n }`; the builder, given the collapsed set, emits a folded directory's `Dir` header (chrome, `▸` glyph) + one `CollapsedFiles` placeholder instead of its `File` rows
- [x] 2.2 `window`/`file_at_row`/digit spread walk the navigable sequence = visible `File` rows + `CollapsedFiles` placeholders (skip `Dir` headers); digits target files only; **reveal targets the selected row** — the placeholder's row when a placeholder is selected, not only a file's
- [x] 2.3 Tests: a folded directory yields header + one placeholder; navigation lands on the placeholder; a selected placeholder scrolled off-screen is revealed; clicking the header or placeholder targets that directory

## 3. Plan over visible files (the diff body)

- [x] 3.1 `Plan::build` takes the collapsed set; skip files whose parent is folded (no header, no hunks); emit one `CollapsedFiles` body row per folded directory
- [x] 3.2 Re-index `file_starts` over the **visible** files: carry a parallel `visible_files: Vec<usize>` (original indices in order); `file_starts[k]` is the row of the k-th visible file
- [x] 3.3 `cycle_mode`/layout rebuilds and the scroll percentage / sticky header read the visible-file plan; rebuild the plan when the collapsed set changes
- [x] 3.4 Tests: a folded directory contributes no file rows (only its placeholder); golden visible-file row sequence; scroll % reflects the reduced rows

## 4. Navigation over visible files

- [x] 4.1 `next_file`/`prev_file`/`current_file` walk `visible_files` (skip folded); `next_file` from the last visible file before a fold lands on the placeholder, then on the first file after
- [x] 4.2 **Jump-by-path unfolds:** the fuzzy file-jump palette, sidebar click, and `open_commit` `land_path` target a file by path — if it is in a folded directory, unfold that directory and reveal the file (vs. step-nav which only walks visible files)
- [x] 4.3 next-unviewed walks visible files only; when nothing visible is unviewed but folded directories hold unreviewed files, it reports "none in view (N hidden in folded dirs)" rather than a bare "none remaining"
- [x] 4.4 Tests: navigation across a fold (file → placeholder → next file); next-unviewed skips folded directories and surfaces the hidden count; fuzzy-jump to a folded file unfolds it

## 5. Fold toggling + keymap

- [x] 5.1 `toggle_fold` (context-sensitive): on a `File` fold its directory and move the cursor to the new placeholder; on a `CollapsedFiles` unfold and move to the directory's first file. `fold_all` (collapse/expand all)
- [x] 5.2 Auto-collapse hook: in `toggle_viewed`, if the file's directory just became fully reviewed, add it to the collapsed set once (do not re-add on redraw); the auto-fold hides the just-reviewed file, so **advance the cursor to the next unviewed file** (or the new placeholder if none remain)
- [x] 5.3 Add the `z` (toggle fold) and `Z` (all) bindings + help entries in `keymap.rs`; route in `handle_key`; inert in the flat view; the hints↔help consistency test stays green
- [x] 5.4 Mouse: a click on a `Dir` header or `CollapsedFiles` placeholder toggles that directory's fold
- [x] 5.5 Tests: `z` folds/unfolds with the right cursor landing; finishing a directory auto-folds; a manual unfold of an auto-folded directory persists

## 6. Rendering

- [x] 6.1 Render `CollapsedFiles` in the sidebar (a selectable `▸ N files` line under the dim directory header; highlight when selected) and in the body (a `▸ <dir> — N files hidden` row; `✓` when the fold was a completed directory)
- [x] 6.2 Visual check (TestBackend): a folded directory shows the header + placeholder in the sidebar and the placeholder (not the files) in the body

## 7. Wrap-up

- [x] 7.1 `cargo clippy --all-targets` clean; full `cargo test` green; `just install`
- [x] 7.2 Manual dogfood: a large multi-directory review → fold a directory (`z`, it leaves both panes), navigate onto and expand a placeholder, mark a directory's last file reviewed (auto-folds), `Z` collapse/expand all, switch views and return (folds restored)
