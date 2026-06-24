## 1. Git/diff loads for the peek

- [x] 1.1 Add a helper to load a file's blob text at a rev (reuse `rev_tree` + `blob_at_path`/`text_of_oid`) and to read its working-copy text
- [x] 1.2 Add a single-file `Changeset` builder for **content mode**: one `DiffFile` whose hunks are a single all-`Context` hunk over the file's lines (full content, highlighted, no +/- markers)
- [x] 1.3 Add a single-file `Changeset` builder for **diff mode** using `compute_hunks_with_context(old, new, context)` at a given context level
- [x] 1.4 Handle binary / missing-side: report "no content"/"no diff" rather than rendering garbage
- [x] 1.5 Unit-test the content builder (all lines Context) and the diff builder at two context levels

## 2. TOP resolution

- [x] 2.1 Add a way to resolve `TOP` for the current view: working copy for local/staged, target commit for a range, the commit for a single commit/show
- [x] 2.2 Expose the current view's commit/old-side so `p` knows the "viewed commit" and `=` knows the change's base
- [x] 2.3 Unit-test TOP resolution per view kind

## 3. Peek state & mode building

- [x] 3.1 Add `Peek { path, origin_local, mode: Content|Diff, context, cs: Changeset, scroll }` and `peek: Option<Peek>` on `App`
- [x] 3.2 `open_peek_preview(path)` (key `p`): build content `cs` for the file at the viewed commit; mode = Content; set `origin_local` from the current view
- [x] 3.3 `open_peek_review(path)` (key `=`): build diff `cs` (base→TOP) at an expanded context; mode = Diff
- [x] 3.4 `peek_toggle_mode` (Tab): switch Content⇄Diff, rebuilding `cs`; for `p` the diff side is viewed-commit→TOP, for `=` it is base→TOP
- [x] 3.5 `peek_context(delta)` (`=`/`-`): clamp and rebuild the diff `cs` at the new context level
- [x] 3.6 `peek_scroll`/close helpers; reset scroll when the content changes

## 4. Highlight slot

- [x] 4.1 Request highlighting for the peeked file under a reserved index (e.g. `usize::MAX`) and read it back the same way
- [x] 4.2 Reset that slot (clear + epoch) whenever the peek's content changes (open/close/mode/context)
- [x] 4.3 Test: opening the peek produces a highlighted result for its file

## 5. Rendering

- [x] 5.1 Build the peek's `Plan` from its one-file `Changeset`; render it in a bordered full-area box reusing `render_row`/`body_spans`
- [x] 5.2 Header line: file path · mode · context level; border/header use the source accent (blue local, commit accent otherwise)
- [x] 5.3 Content mode shows a single line-number column; diff mode shows the normal gutter/markers
- [x] 5.4 Empty diff (`p` from a non-commit view) renders "no differences"; binary renders "no preview"
- [x] 5.5 Render test: peek frame shows the file and the mode/accent

## 6. Input handling

- [x] 6.1 While the peek is open, capture all keys: `Tab` toggle, `=`/`+` expand, `-`/`_` compact, scroll keys, `Esc` close
- [x] 6.2 Bind `p` and `=`/`+` in the main stream (on the selected file) to open the peek
- [x] 6.3 Ensure `=`/`-`/`Tab` don't conflict with existing main-view bindings
- [x] 6.4 Tests: `p` opens content; `=` opens diff with expanded context; `Tab` toggles; `Esc` closes; context +/- changes hunk size

## 7. Wrap-up

- [x] 7.1 Add peek keys (`p`, `=`/`-`, `Tab`) to the `?` help overlay
- [x] 7.2 `cargo test`, `cargo clippy` clean; dogfood preview, diff-vs-TOP, context expand/compact, from both local and commit views
