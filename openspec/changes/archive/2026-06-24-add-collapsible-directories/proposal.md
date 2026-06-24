## Why

Directory grouping orients you in a changeset, but on a large diff you still
scroll past directories you don't care about — generated code, vendored deps, a
subsystem you're not reviewing — in both the sidebar and the diff stream. And in
a review, finished directories keep taking space. Collapsing a directory should
take its files *out of scope*: gone from the list, gone from the diff body,
skipped by navigation — so you can fold away what you're done with (or never
cared about) and focus the review on what's left.

Two triggers: a directory **auto-folds** the moment its last file is marked
reviewed (the review loop becomes "finish a directory → it vanishes → the list
is what remains"), and you can **manually fold** any directory to drop it from
scope. The fold must be fully keyboard-driven, not mouse-only.

## What Changes

- **A per-view collapsed-directory set.** Which directories are folded is per
  view, stored in `ViewState` beside `scroll`/`selected`/`viewed`, so it is
  saved and restored as you move through the view history.
- **A collapsed directory replaces its files with one selectable placeholder.**
  The directory header stays non-selectable chrome (as every dir line is); its
  file rows collapse to a single `▸ N files` placeholder — in the sidebar *and*
  as a row in the diff body. The directory's files are excluded from the diff
  stream entirely (no headers, no hunks), so collapse genuinely reduces scope.
- **Selection spans files and collapsed placeholders.** The cursor (`selected`)
  becomes "a file *or* a collapsed-directory placeholder"; `j`/`k`, clicks, and
  jumps walk files plus placeholders (directory headers are skipped, as today).
  File actions (`v`, peek, the `1–9` digits) apply to files only and are inert on
  a placeholder, whose one verb is expand.
- **Fold toggling, keyboard-first.** `z` toggles the fold context-sensitively —
  on a file it folds that file's directory (cursor lands on the new placeholder);
  on a placeholder it expands (cursor lands on the directory's first file). `Z`
  collapses/expands all. A mouse click on the directory line or the placeholder
  does the same. Per-line (folding `src/tui` does not fold `src/tui/widgets`);
  collapse applies only in the directory-grouped view.
- **Auto-collapse on completion.** Marking a directory's last file reviewed folds
  it (a one-time edge — re-expanding by hand sticks).
- **Scope semantics.** Collapsed files are excluded from the diff body and skipped
  by file navigation and next-unviewed; the overall reviewed count still includes
  them (collapse is a view filter, not deletion); the scroll percentage tracks the
  visible rows.

The notable internal consequence: the diff body's `Plan` — today built over every
file — is rebuilt over the **visible** files (collapse-filtered), and file
navigation re-indexes from "all files" to "visible files". Grouping left the body
untouched; this is what reaches into it.

## Capabilities

### New Capabilities
- `directory-collapse`: fold a directory (manually, or automatically when its
  last file is reviewed) so its files leave both the sidebar list and the diff
  body, replaced by a single selectable placeholder, with the fold fully
  keyboard-navigable and persisted per view.

## Impact

- **`src/tui/view.rs`** — `ViewState` gains the collapsed-directory set (kept in
  history); `Selection` becomes `File | CollapsedFiles(dir)`.
- **`src/tui/sidebar.rs`** — `SidebarRow` gains `CollapsedFiles(dir, n)`; the row
  builder emits a folded directory's header (chrome) + one placeholder instead of
  its files; navigation/window/digits walk files + placeholders.
- **`src/tui/rows.rs`** — `Plan::build` takes the collapsed set, builds over the
  visible files, and emits a `CollapsedFiles` body row per fold; `file_starts`
  becomes a visible-file index (parallel `visible_files`).
- **`src/tui/app.rs` / `stream`** — file navigation (`next_file`/`prev_file`/
  `current_file`/`jump_to_file`) walks visible files; the selection enum threads
  through; `toggle_grouping`-style `toggle_fold`/`fold_all`; the auto-collapse
  hook on `toggle_viewed`; next-unviewed over visible files.
- **`src/tui/ui.rs`** — render the `CollapsedFiles` placeholder (sidebar + body),
  selectable/highlightable.
- **`src/tui/keymap.rs` / `mod.rs`** — `z` / `Z` bindings + help entries (the
  hints↔help consistency test will require them documented).
- **Risk** — the selection model changes from a bare file index to a `File | Dir`
  enum, threading through navigation, the sidebar highlight, the body scroll-to,
  and every file-action (made inert on a placeholder); and `file_starts`
  re-indexes over visible files. This is the genuinely invasive part.
