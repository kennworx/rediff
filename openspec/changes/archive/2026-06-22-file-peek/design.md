## Context

rsdiff renders a `Changeset` (a list of `DiffFile`s with hunks) through a flat row plan (`Plan`/`SplitPlan`), an async highlight worker keyed by file index, and a windowed stream renderer. The commit-navigation change added a browser-style view stack (`ViewEntry` with `kind`, `cs: Rc<Changeset>`, `review`, scroll/selection). Diff context is fixed at 3 lines, but `diff::compute_hunks_with_context(old, new, context)` already exists. The git layer can load any blob (`blob_at_path`, `text_of_oid`, `rev_tree`) and read the working tree (`read_worktree`).

The peek is a focused, single-file overlay for getting context while reviewing — see a whole file, or compare a file's version against the top of what you're reviewing — without disturbing the main view or the view history.

## Goals / Non-Goals

**Goals:**
- A modal, full-area, scrollable, highlighted view of exactly one file.
- Two modes toggled in place: **content** (whole file, no diff) and **diff** (a unified diff with adjustable context).
- Two open keys whose diffs differ only in their start point, both ending at `TOP`:
  - `p` (history): file at the viewed commit; diff `commit → TOP`.
  - `=`/`+` (review): file at `TOP`; diff = the view's own change (`base → TOP`).
- `=`/`-` adjust the diff context level live.
- Reuse the existing renderer/highlight/scroll; inherit the source accent (blue/magenta).

**Non-Goals:**
- No viewed-tracking — the peek is an overview helper, not a review surface.
- No entry in the `<`/`>` view-history stack — it is ephemeral (`Esc` returns you exactly where you were).
- No multi-file navigation inside the peek — it is one file at a time.
- No editing, staging, or any repo mutation.

## Goals / Non-Goals — `TOP`

`TOP` is the newest side of the current review context, computed from the active view:

| active view | `TOP` |
|---|---|
| working-tree / staged review | the working copy |
| `review base..target` (range) | the target commit |
| a single commit / `show` | that commit |

`p`'s diff compares the drilled-into commit's version of the file against `TOP` (so in a branch review it measures against the branch top, not an unrelated dirty worktree). `=`'s diff is the view's own change, which always ends at `TOP`.

## Decisions

### Decision 1: A modal overlay, not a view-stack entry
The peek is transient state on `App` (`peek: Option<Peek>`), not a `ViewEntry`. Opening it does not push history; `Esc` drops it and restores the prior frame unchanged. Mode/context changes mutate the `Peek` in place and never touch the view stack.

```
Peek {
    path: String,
    origin_local: bool,          // source accent: blue if true, else magenta
    mode: Content | Diff,
    context: usize,              // diff context level (Diff mode)
    cs: Changeset,               // a synthetic ONE-file changeset for the current mode
    scroll: usize,
}
```

**Why modal over stack:** the user wants a "popup … for this file only" that returns in place; mode/context toggling would otherwise spam the history with entries. **Alternative considered:** push preview/diff as single-file views onto the stack — rejected; it pollutes history and conflates a context-helper with navigable review state.

### Decision 2: Reuse the renderer over a one-file `Changeset`
Each mode builds a synthetic `Changeset` with a single `DiffFile`, and the peek renders it with the existing `Plan` + `render_row`/`body_spans` path inside a bordered full-area box. So highlighting, line numbers, intra-line emphasis, and scrolling come for free.

- **Content mode** — a builder produces a `DiffFile` whose hunks are one synthetic hunk with every line as `Context` (full file, no `+/-`). `compute_hunks(x, x)` yields nothing, so this is a dedicated builder, not the diff path.
- **Diff mode** — `compute_hunks_with_context(old, new, context)` with the mode's two sides; `=`/`-` change `context` and rebuild.

**Why:** maximal reuse; the only genuinely new rendering is the overlay frame + a header line (file · mode · context).

### Decision 3: The two open keys and their sides

| key | opens in | content side | diff old → new |
|---|---|---|---|
| `p` | Content | file @ viewed commit | `viewed commit` → `TOP` |
| `=` / `+` | Diff (context bumped) | file @ `TOP` | the view's change: `base` → `TOP` |

`p` is meaningful from a commit/history context; in a plain working-tree view its `commit → TOP` diff is empty, so it is effectively just a preview. `=` is available from any view (there is always a current change).

### Decision 4: Highlight slot for the peeked file
The async `HlService` caches by file index within the current changeset; the peek's file is not in that changeset. The peek requests highlighting under a reserved index (e.g. `usize::MAX`) and reads it back the same way, and resets that slot when the peek's content changes (mode/context/open/close) using the existing clear-and-epoch mechanism. **Alternative:** a second `HlService` — rejected as overkill; one reserved index suffices.

### Decision 5: Keys and capture
While the peek is open it captures all input:

| key | action |
|---|---|
| `Tab` | toggle Content ⇄ Diff |
| `=` / `+` | expand diff context (Diff mode) |
| `-` / `_` | compact diff context (min ~0–3) |
| `j`/`k`, `↑`/`↓`, `PgUp`/`PgDn`, `g`/`G` | scroll the peek |
| `Esc` (and `p`/`q`?) | close |

Opened from the main stream with `p` (preview) or `=`/`+` (review diff) on the selected file. `=`/`-` and `Tab` are otherwise unused in the main stream, so no conflict. `=` from the main view both opens the peek and sets Diff mode with context above the main view's 3.

### Decision 6: Source color, no tracking
The overlay border/header use the origin view's accent — blue for a local/staged origin, magenta for a commit/range origin (`origin_local`). No `viewed` vector; `v`/`u`/`✓` are inert and irrelevant in the peek.

## Risks / Trade-offs

- **Renderer coupling** (render fns assume the main `App`'s plan/scroll) → Build the peek's `Plan` from its one-file `Changeset` and pass the peek's scroll/area explicitly; factor a small shared "render this plan in this rect" helper if the stream renderer is too entangled with `App`.
- **Highlight slot collision** with a real file at the reserved index → Use an index no real changeset reaches (`usize::MAX`) and reset on content change; covered by a test that opens the peek and waits for its highlight.
- **`p` from a non-commit view yields an empty diff** → Acceptable (it degrades to a plain preview); show "no differences" rather than a blank diff, and consider hiding `Tab`'s diff affordance there.
- **Context level unbounded** → Clamp `context` to `[0, file_len]`; at max it's the whole file with markers (a useful "everything" view), at min a tight hunk view.
- **Binary / missing side** → Binary → "no preview"; a side absent at the chosen rev → render as an all-add/all-remove diff or an empty content view, consistent with how the main diff handles add/delete.

## Open Questions

- Should `q` also close the peek (in addition to `Esc`), or be reserved so a stray `q` doesn't quit the app from within the overlay? Default: `Esc` closes; `q` is ignored inside the peek.
- Does the content mode show one line-number column (the file's own numbers) — yes — or mimic the diff gutter? Default: a single number column.
