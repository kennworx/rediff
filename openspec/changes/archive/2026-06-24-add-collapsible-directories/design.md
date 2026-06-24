## Context

Directory grouping (the `add-sidebar-dir-grouping` change) is a **sidebar-only
display**: the diff body (`Plan::build` in `rows.rs`) is untouched and shows every
file's header + hunks in path order, and `selected` is a bare file index that
drives the diff scroll, the sidebar highlight, the jump digits, review actions,
and the peek. Reviewed files already collapse their *hunks* to a single
`Row::Collapsed(n)` placeholder in the body, but the file still appears and
`file_starts` has one (dense) entry per file.

Collapsing a *directory* with scope-reduction reopens two things grouping left
alone: the **diff body** (collapsed files must leave the `Plan`) and the **cursor
model** (the fold placeholder must be reachable and expandable by keyboard).

## Goals / Non-Goals

**Goals:**
- Fold a directory so its files leave both the sidebar list and the diff body,
  replaced by one selectable `▸ N files` placeholder.
- Both triggers: auto-fold when a directory's last file is reviewed; manual fold
  to drop scope. Fully keyboard-driven (`z`/`Z`), plus mouse.
- Per-view collapse state, persisted in the view history.
- Keep directory *headers* as non-interactive chrome; the placeholder is the
  selectable element.

**Non-Goals:**
- Prefix/subtree folding (folding `src/tui` does not fold `src/tui/widgets`) —
  per-line, consistent with the flat grouping model.
- A nested, indented tree.
- Making expanded directory headers selectable (only collapsed placeholders join
  files as cursor stops).
- Collapse in the flat (non-grouped) view — there are no directories there; the
  fold keys are inert.

## Decisions

### Decision 1: Per-view collapsed set, in `ViewState`
The set of folded directory paths lives in `ViewState` alongside `scroll`,
`selected`, `viewed` — so it round-trips through the view history exactly like
the rest of the per-view state (back/forward/home restore each view's folds). A
file is hidden iff its parent directory is in the set (per-line: only the exact
parent, not ancestors).

### Decision 2: A folded directory = header chrome + one selectable placeholder
A directory line is **always** non-selectable chrome (expanded or collapsed). The
sidebar row model gains a third variant:
```
enum SidebarRow { Dir(String), File(usize), CollapsedFiles { dir: String, n: usize } }
```
When a directory is folded, the builder emits its `Dir` header (chrome, with a
`▸` glyph) followed by **one** `CollapsedFiles` placeholder instead of its `File`
rows. The placeholder is the selectable/expandable element. (A folded directory
is therefore two sidebar lines, not one — the price of keeping the header
non-selectable; still a large save over header-plus-N-files.) This mirrors the
existing reviewed-file `Collapsed` placeholder pattern.

### Decision 3: Selection is a file *or* a collapsed placeholder
```
enum Selection { File(usize), CollapsedFiles(String) }   // String = the folded dir
```
`j`/`k` (and clicks, and the back/forward of navigation) walk the **navigable
sequence** = visible `File` rows + `CollapsedFiles` placeholders; `Dir` headers
are skipped (as they are today). On a `CollapsedFiles` selection the file-only
actions — `v` (toggle reviewed), `p`/`=` (peek), the `1–9` jump digits (which
only ever target files) — are **inert**; its one verb is expand. The diff body
scrolls to the placeholder's row and the sidebar highlights it.

This is the "node cursor" scoped to collapsed directories only — the minimum that
makes expand keyboard-reachable. It is the main model cost: every reader of
`selected` learns to handle "or a directory" (mostly: act only on `File`,
otherwise inert).

**Revealing a placeholder selection.** The sidebar window/reveal targets the
selected *row* — for a `CollapsedFiles` selection that is the placeholder's row,
not a file's. So reveal works on `Selection` (the row of the file *or* the
placeholder), not only on a file index.

**Leaving the grouped view.** Toggling to the flat view (`D`) while a
`CollapsedFiles` placeholder is selected has no placeholder to land on, so the
selection converts to that directory's first file. (Folds are kept in the
collapsed set and restored when grouped view returns.)

### Decision 4: The `Plan` is built over the visible files (the main lift)
`Plan::build(cs, viewed, layout, collapsed)` skips files whose parent is folded:
no `FileHeader`, no hunks. For each folded directory it emits **one**
`CollapsedFiles` body row (a `▸ src/api — N files hidden` line, the body twin of
the sidebar placeholder, selectable). Consequences:
- **`file_starts` becomes a visible-file index.** Today `file_starts[i]` is the
  row of file `i` for all `i`; with folds, hidden files have no row. Carry a
  parallel `visible_files: Vec<usize>` (original indices, in order) and index
  `file_starts` by *visible ordinal*; navigation walks `visible_files`.
- **The scroll percentage and sticky header** read the visible-file `Plan`, so
  they reflect the reduced scope.
- The `CollapsedFiles` body rows are chrome interleaved between visible files —
  not `file_starts` entries — and are the body's only directory markers (expanded
  directories still have no body header; a marker appears only where content is
  hidden).

**Jumping *to* a folded file unfolds it.** `next_file`/`prev_file` walk visible
files (they never target a folded file), but the fuzzy file-jump palette, a
sidebar click, and `open_commit`'s `land_path` select a file *by path* — which may
be inside a folded directory. Selecting such a file **unfolds its directory** (and
reveals it) rather than skipping or no-opping. So "jump to a file" always lands on
that file, expanding whatever was hiding it.

### Decision 5: `z` toggles the fold context-sensitively; `Z` all; click
- **`z`** on a `File` → fold that file's directory; the cursor lands on the new
  `CollapsedFiles` placeholder (so `z` again undoes it). `z` on a
  `CollapsedFiles` → expand; the cursor lands on the directory's first file.
- **`Z`** → collapse-all / expand-all (toggle by whether anything is currently
  expanded).
- A mouse click on a `Dir` header or a `CollapsedFiles` placeholder toggles that
  directory's fold.
Both keys are inert in the flat view (no directories). They are defined in the
keymap table with help entries, so the hints↔help consistency test requires them
documented.

### Decision 6: Auto-collapse is a once-edge; manual expand sticks
`toggle_viewed` checks, after the toggle, whether the affected file's directory is
now **fully** reviewed; if it just became so, the directory is added to the
collapsed set (once, on the completion edge). It is *not* re-added on every
redraw, so re-expanding a finished directory by hand stays expanded. Auto-collapse
applies only in a review session and only in the grouped view.

**Cursor after an auto-fold (the review-loop case).** Marking a directory's last
file reviewed hides the file the cursor was on. To keep the review flowing, the
cursor then **advances to the next unviewed file** (the same landing as
next-unviewed) rather than parking on the new placeholder — so "finish a directory
→ it folds → you're already on the next thing to review." If no unviewed file
remains in scope, the cursor falls back to the new placeholder.

### Decision 7: Scope semantics
- **Progress counts collapsed files.** The reviewed count `X/N` is over *all*
  files; a folded-because-done directory contributes its files to both `N` and
  `X`. Collapse is a view filter, not exclusion — so finishing a directory does
  not shrink the denominator.
- **Navigation and next-unviewed walk visible files only.** `next_file`/
  `prev_file` and next-unviewed skip files in folded directories; if everything
  *visible* is reviewed, next-unviewed reports "none remaining" even if a manually
  folded directory still holds unreviewed files (they are deliberately out of
  scope). To avoid the confusing `38/40` + "none remaining" combination, when
  next-unviewed finds nothing visible but unreviewed files remain in folded
  directories, it SHALL say so — "none in view (N hidden in folded dirs)" — so the
  user knows the remainder is folded away, not lost.
- **Selection on fold → nearest visible.** Folding the directory of the selected
  file moves the cursor to the resulting placeholder (Decision 5); folding via
  `Z`/elsewhere that hides the selected file moves it to the nearest still-visible
  node.

## Risks / Trade-offs

- **The selection enum is the invasive change.** `selected: usize` → `Selection`
  threads through `stream` navigation, the sidebar highlight/hit-test, the body
  scroll-to, and every file-action (guarded to act only on `File`). Mitigation:
  most sites become `if let Selection::File(i) = sel { … } // else inert`; cover
  the inert-on-placeholder cases with tests.
- **`file_starts` re-indexing.** Moving from dense (all files) to visible-ordinal
  is the other structural change; a `visible_files` map keeps it explicit. Test
  that navigation lands correctly across a fold.
- **Two collapse mechanisms coexist.** Per-file reviewed hunk-collapse (a file
  stays, hunks → placeholder) and per-directory fold (files leave entirely). A
  partially-reviewed directory shows its files (reviewed ones as hunk
  placeholders); the moment the last lands, the whole directory folds. Assert the
  transition.
- **Folded directory is two sidebar lines** (header + placeholder). Accepted to
  keep headers non-selectable; still a large vertical save.

## Open Questions

- Should `Z` (collapse-all) fold *every* directory or only fully-reviewed ones?
  Modeled as collapse-all here; a "collapse reviewed" variant could be a third
  binding if wanted.
- Should the `CollapsedFiles` placeholder show an aggregate (reviewed `4/4 ✓`,
  summed `+/-`) beyond the count? Cheap to add; ships with the count + a `✓` when
  the fold was automatic (fully reviewed).
