## Context

rsdiff is a viewer-only TUI git-diff browser. Today the lifecycle is: `main.rs` resolves the CLI, calls `git::load()` once into an owned `Changeset`, drops the repo, and hands `tui::run(&cs, …)` a borrow that lives for the whole session. `App<'a>` holds `cs: &'a Changeset`, and `plan`, `split`, `viewed`, and the highlight cache are all derived from that single borrow. The fuzzy file palette (`Palette` + `open_palette`/`palette_input`/`palette_move`/`palette_pick`/`palette_confirm`) already implements a filtered popup with number shortcuts.

This change adds the ability to switch what is being viewed *at runtime* — pick a commit, dive into a file's history, step back/forward — and to review a commit or branch range. The feature description hides the real cost: a borrow cannot be replaced at runtime, so the central work is an ownership refactor, not the popup.

## Goals / Non-Goals

**Goals:**
- Switch the displayed changeset at runtime without quitting.
- Browser-style back/forward over a trail of views, each restoring its own scroll/selection.
- Review a commit or a branch range with the same viewed-tracking as local changes; review progress survives browsing into history.
- Reuse the existing palette pattern for the commit picker.
- Stay within the speed budget (sub-ms picker open, reuse the existing tree-to-tree path for diffs).

**Non-Goals:**
- No checkout, staging, cherry-pick, or any repo mutation — strictly viewer-only.
- No per-commit sections in the review surface; a range review is a single **combined net diff** (we may revisit per-commit later).
- No lazy/paged commit loading — a fixed cap (200) loaded eagerly is enough.
- No persistence of view history or review progress across process runs.

## Decisions

### Decision 1: A browser-style **view stack**, not a flat changeset swap
The requirements (review progress persists across browsing, `{`/`}` back/forward, `C` back to home) collectively force a stack of views with a cursor, not a single replaceable changeset.

```
ViewEntry {
   kind: Local | Staged | Commit(id) | Range(base..target),
   source_label: String,
   cs: Rc<Changeset>,
   scroll: usize,
   selected: usize,
   review: Option<Vec<bool>>,   // Some = review session (v/u/✓ active) · None = browse
}
```

- `App` owns `views: Vec<ViewEntry>` and `cursor: usize`; the displayed view is `views[cursor]`.
- `c`/`F` push a new entry, **truncating any forward history** past the cursor (browser semantics).
- `{` = cursor−1, `}` = cursor+1 (clamped). `C` = jump cursor to the home entry (index 0) **iff** index 0 is `Local`/`Staged`/a review; inert otherwise.
- Each entry restores its own `scroll`/`selected` when it becomes current.

**Why over a flat swap:** a single `cs` field plus a separate `viewed` cannot express "go back to where I was, with my review progress intact." Alternatives (a separate "saved local state" struct) collapse into this stack anyway once back/forward exists.

### Decision 2: `Rc<Changeset>`, dropping `App<'a>`'s lifetime
`App` stops borrowing. Each `ViewEntry.cs` is an `Rc<Changeset>`. `Plan::build`, `SplitPlan::build`, and `HlService::request` already take `&Changeset`, so they consume `&view.cs` unchanged — only `App`'s construction signatures and the ~30 test call sites change (mechanical).

**Why `Rc` over a bare move:** the highlight worker references file text, and back/forward wants cheap "hold the previous changeset"; `Rc` makes both a clone of a pointer. **Alternative considered:** reload each view from the repo on every navigation (no caching). Rejected for back/forward — even a <5ms reload loses the per-view scroll restore and adds flicker; caching the `Rc` is simpler and instant.

### Decision 3: Review is a **per-view property**, not a mode of `App`
`review: Option<Vec<bool>>` lives on each entry. `v`/`u`/`✓`/collapse act on `views[cursor].review` when `Some`, inert when `None`. Launch decides the home entry's value:

| launched as | home kind | review | color |
|---|---|---|---|
| `rsdiff diff` (default) | Local | `Some` | blue |
| `rsdiff diff --staged` | Staged | `Some` | blue |
| `rsdiff review [sha] [--from base]` | Commit/Range | `Some` | green |
| `rsdiff show <ref>` | Commit | `None` | green |
| `c` / `F` dive (in-TUI) | Commit | `None` | green |

`R` promotes the current browse entry: attach a fresh `vec![false; files]`, flip to a review session. This subsumes the earlier "viewed lives on the local entry" idea — a reviewed commit then behaves identically to local review.

**Why per-view:** the user wants to review commits too, and to keep local review progress while browsing. A global `viewed` cannot represent two independent review surfaces; a per-view optional does, for free.

### Decision 4: `review` semantics — combined net diff via merge-base
`rsdiff review [sha] [--from base]`:
- no `sha` → target = HEAD; `sha` given → target = that commit.
- no `--from` → review the single commit (target vs `target^`), reusing `show`.
- `--from base` → review the **range as one combined net diff**: `diff_tree_to_tree(merge_base(base, target).tree, target.tree)`.

The **commit list** for the range (used to scope the `c` picker) is the two-dot rev-walk `base..target`; the **diff** uses the merge-base (three-dot) so a `base` that moved ahead after branching does not show its divergent commits inverted. They coincide when `base` is an ancestor of `target` (the common case).

**Why merge-base for the diff:** it is the GitHub "Files changed" behavior and the only one that shows *only what the branch did*. **Alternative:** literal `base..target` two-dot tree diff — rejected, it leaks `base`'s post-branch commits into the review.

### Decision 5: Smart picker filter with three interpretations
One input box, dispatched by what is typed:

```
query → hex & len ≥ 4              → SHA prefix-match the loaded commit list
      → matches a known repo path  → file-scoped list: commits that touched it
      → otherwise                  → fuzzy subsequence over commit summary (reuse fuzzy::score)
```

"Matches a known repo path" = prefix-matches a path present in the current changeset or HEAD's tree. The explicit, unambiguous route to a file-scoped list is `F` on the selected file; the path-detection in `c` is a convenience.

**File-scoped log is cheap:** for each walked commit (≤200), compare the blob oid at `path` in `commit.tree()` vs its parent's tree; differing (or presence flip) means the commit touched it. Two tree lookups per commit, no full tree-diff.

### Decision 6: Highlight cache — drop and bump an epoch on switch
On any view switch: `hl.clear()` and `hl.epoch += 1`. Each request is tagged with the current epoch; drained results whose epoch is stale are discarded. Visible files re-highlight asynchronously off the UI thread (single-digit ms with tree-sitter), so the worst case is a sub-frame color flicker, never a stall.

**Why not a per-view generation-keyed cache:** premature for a viewer. The bare-index cache would otherwise serve the old view's "file 3" highlight for the new view's "file 3"; one `u32` epoch closes that hole. `HlService::set_dark` already invalidates the cache, so the clear path largely exists.

### Decision 7: Source color coding
Introduce a per-view source accent: blue for `Local`/`Staged`, green for `Commit`/`Range`. Applied in the status-line source label and the sidebar file markers (selection accent + digit badges). Reuse existing theme colors where possible (`added` green, an existing blue/`accent`) and ensure both dark and light themes carry the pair. The status line also distinguishes a review session (shows `✓ n/m`) from a browse view (no counts).

### Decision 8: Key handling for punctuation/shifted keys
`{`, `}`, `C`, `F`, `R` arrive from crossterm as `Char('{')`, `Char('}')`, `Char('C')`, `Char('F')`, `Char('R')` — the shift is baked into the character, not a `KeyModifiers::SHIFT` flag. The handler matches these characters directly. `[`/`]` stay hunk-nav; `{`/`}` are view back/forward (a deliberate parallel).

### Decision 9: A range review excludes its own commits from pickers
While the current view is a **range review** (`review --from base`), the commit selection dialogs (`c` and `F`) SHALL exclude every commit that belongs to the reviewed range — the two-dot `base..target` set. The range *is* the net diff under review, so the picker is for reaching history *outside* it (the merge-base and earlier), not for re-listing what is already being reviewed.

Implementation: the range set `base..target` is already known (it scopes the range and is computed from the rev-walk). Hold those ids as a `HashSet`; when building a picker list during a range review, filter out any commit whose id is in the set. This applies to both the unscoped `c` list and the file-scoped `F` list. Outside a range review (local, single-commit, or browse views), no exclusion applies.

**Why:** re-offering the range's commits is redundant with the review surface and clutters the path to surrounding context. **Alternative considered:** scoping `c` *to* the range's commits (the earlier draft) — rejected; that duplicates the review surface and hides the broader history the picker is meant to reach.

## Risks / Trade-offs

- **Ownership refactor churn** (App lifetime + ~30 test sites) → Mechanical; do it first as its own task so everything else builds on the stable shape, and keep the diff focused.
- **Borrow conflicts** mutating `App` while reading `views[cursor].cs` for rendering → Hold the displayed `Rc<Changeset>` by clone for the draw, or split the data so navigation mutates indices/scroll and rendering reads the `Rc`. The `Rc` clone is the escape hatch.
- **Path-detection false positives** in the `c` filter (a summary word that happens to match a path) → Keep `F` as the reliable explicit route; only treat a query as a path when it prefix-matches an actual known path; show the active filter mode in the popup so the interpretation is visible.
- **Merge-base edge cases** (unrelated histories, `base` not an ancestor, shallow clones) → Fall back to a literal two-dot tree diff when no merge-base exists, and surface the range label in the status so the user sees what was compared.
- **Late highlight results** painting the wrong view → The epoch guard (Decision 6) is the mitigation; cover it with a test that switches views with an in-flight request.
- **`C`/`v`/`R` availability confusion** → Grey out unavailable actions in the status/help when the home view is a commit (`C` inert) or the current view is a browse (`v`/`u` inert).

## Open Questions

- Should `R` (promote-to-review) on a *range* browse view be allowed, or only on single-commit browse views? Default: allow it, attaching a fresh per-file `viewed` to whatever the current changeset is.
- Picker scope outside a range review: recent history from HEAD (cap 200) vs from the current view's commit. Default: from HEAD, so the picker is a stable "jump anywhere" affordance regardless of where you have browsed. (During a range review, the range's own commits are excluded per Decision 9.)
