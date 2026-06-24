## Why

The `tui` layer has accreted into a shape that resists change. `App` is a ~40-field, ~90-method god object whose live navigation state is a hand-synced denormalized copy of the per-view state; the layout (`Plan`/`SplitPlan`) and the single-file peek each duplicate the stream's machinery; and there is no first-class notion of "mode," so the keyboard router, the mouse router, the status line, and the help overlay each re-derive "what mode am I in" from raw flags — inconsistently. Every new feature widens `App` and must be implemented up to four times.

This is not only a maintainability concern. The same structural gaps produce concrete, user-visible defects:

- **A latent panic.** Switching away from a working-tree review mid-load and back re-enumerates the working tree; if files were added meanwhile, `next_unviewed` indexes the old-length `viewed` vector out of bounds and the program panics.
- **Wasted work.** Abandoning a load discards all completed diffs; returning re-diffs every file from scratch (switch away at 99 % → redo 100 %).
- **A lying status bar.** Opening the file peek (or help) leaves the bottom status line showing the *stream's* keys and scroll position — advertising bindings that do nothing in the peek. The scroll percentage is also computed against the stack plan even in split mode, so it is wrong there.
- **Mouse leaks through overlays.** The mouse router only knows about the peek; scrolling or clicking while the fuzzy palette is open drives the diff *behind* it.

These defects are not, by themselves, the justification — each is independently hot-fixable without the refactor: clamp `viewed` to `cs.files.len()` on resume (the panic, ~3 lines), add the palette/help checks to the mouse arm (the leak, ~1 line), divide by the active plan's row count (the split %, ~2 lines), save+restore `h_scroll`/`wrap` in `save_current`/`load_current` (the leak, ~4 lines). They are listed as **evidence of the rot**, not the payoff. The two things that genuinely *require* the restructuring are (1) **resumable loads**, which need stable per-view file identity that positional state cannot provide, and (2) **not paying for the next four bugs** — today every per-view field, every mode-dependent decision, and every layout branch is duplicated up to four times, so the cost is paid again on each new feature. The refactor's return is the fifth feature costing 1× instead of 4×; the bug fixes fall out of the same two primitives (per-view state with stable identity; a first-class `Mode`) for free.

If the bugs are wanted gone *before* the refactor lands, the four hot-fixes above can ship as a pre-PR; the refactor then deletes them as it subsumes their mechanisms.

## What Changes

A behavior-preserving (except where it fixes the defects above) restructuring of `src/tui/`, in independently shippable phases:

- **Per-view state + snapshot identity** — the live navigation state (`scroll`, `h_scroll`, `wrap`, `selected`, `reveal_selected`, `viewed`) moves into the view entry as one `ViewState`, ending the manual `save_current`/`load_current` round-trip and the `h_scroll`/`wrap` leak across switches. A view's file set becomes **immutable for its lifetime** (the enumeration stubs live on the view): no re-enumeration on return, completed diffs are preserved, and a resumed load finishes only the still-undiffed files. This eliminates the `next_unviewed` panic by construction and turns "re-diff from scratch" into "resume."
- **First-class `Mode` — a base with at most one overlay** — `Mode { base: Normal{focus} | Peek(PeekState), overlay: Option<Palette | Help> }` becomes the single source of truth for keyboard routing, mouse routing, status hints/context, and overlay selection. Help and Palette are *layers over a base* (they remember it: help over `Peek` shows peek keys, over `Normal` shows stream keys), not siblings that forget their context; `Option<Overlay>` makes "exactly one overlay" a type guarantee, killing the three-flags-can-stack hazard. Overlays are pure widgets that **return a result** (a `Palette` confirm emits `Jump`/`OpenCommit`, the loop routes it) rather than reaching into the model. The status bar reflects the active layer; the mouse stops leaking through overlays; the percentage uses the active plan; hints and `?` help render from one keymap table so the three transcriptions can no longer drift.
- **Dissolve `App` into `Session`** — `App` is **removed**, not thinned. The load/view state-machine (the stack + the private `loader`/in-flight flags + the lifecycle) becomes one cohesive struct, `Session`; the shared per-view model is acted on by stateless operation modules (`stream`, `sidebar`, `review`); interaction state lives in `Mode`; `hl`/`theme` are services. The event loop wires these as locals — there is **no replacement root struct**, which is what stops the god object from re-accreting.
- **One parametric `Plan`** — `Plan`/`SplitPlan` and `Row`/`SRow` collapse to one row model with a `layout` parameter, written once; the `split_active` accessor-switching disappears, and the single `Plan` is cached per-view on `ViewEntry`.
- **Peek as the stream** — the peek becomes the `stream` functions over a synthetic one-file changeset (`Base::Peek(PeekState)`), deleting its parallel `peek_*` navigation and dual plans.
- **Pure render** — `draw` becomes a function of `(Mode, Session)` rather than mutating `App` while measuring.

This change touches only `src/tui/` (plus a small `git`/`model` adjustment for view-owned stubs). The diff algorithm, git loading, and highlighting are unchanged.

## Capabilities

### New Capabilities
- `mode-routing`: a single active input mode that deterministically drives keyboard routing, mouse routing, the status line's hints and context, and which overlay is shown — with the keymap defined once and the status hints and help overlay rendered from it.

### Modified Capabilities
- `viewed-tracking`: reviewed state and the active-file cursor follow files by identity, not by position, so a view whose file set changes (a refreshed working tree) keeps each file's reviewed flag aligned and never indexes out of bounds; next-unviewed is panic-free.
- `streaming-diff-load`: a view's enumerated file set is fixed for the view's lifetime; abandoning a load preserves completed diffs and a resumed load re-diffs only the undiffed files (no re-enumeration, no from-scratch redo).

## Impact

- **`src/tui/app.rs`** — the bulk of the change: introduce `ViewState`; move it onto `ViewEntry`; extract `Session` (stack + private load machine + lifecycle); carve `stream`/`sidebar`/`review` out as free-function modules; **delete `App`** (the event loop owns the loop locals, no replacement root).
- **`src/tui/view.rs`** — `ViewEntry` owns `ViewState`, the enumeration stubs (`Arc<Vec<FileStub>>` or the partial `cs`), and the per-view `Plan`-cache, enabling snapshot/resume.
- **`src/tui/mod.rs`** — `handle_key` + the event-loop mouse arm collapse into one router with one precedence (overlay-first, else base-by-focus); a keymap table replaces the inline cascade; the loop wires `Session`/`Mode`/services.
- **`src/tui/rows.rs`** — `Plan`/`SplitPlan` unify into one parametric `Plan`.
- **`src/tui/peek.rs`** — re-expressed as the `stream` functions over a one-file changeset (`Base::Peek(PeekState)`).
- **`src/tui/ui.rs`** — `draw_status` reads the active layer + active plan; overlay dispatch keys off the single `Option<Overlay>`; the status hints and `?` help render from the keymap table; `draw` is a pure function of `(Mode, Session)`.
- **Risk** — this is a large refactor of the most test-covered part of the codebase; the existing TUI test suite is the safety net, and each phase carries its own regression test (notably the `next_unviewed`-panic repro and the resume-only-undiffed assertion). Phases land independently to keep each diff reviewable and each success criterion crisp.
