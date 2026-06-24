## Context

`src/tui/` is the heavy half of rsdiff (~3.7 k LOC across `app.rs`, `mod.rs`, `ui.rs`, `rows.rs`, `peek.rs`). The lower layers (`model`, `diff`, `git`, `highlight`) are clean: one normalized `Changeset`, a pure imara-diff wrapper, a lazy enumerate→stream loader, and a tiered highlighter. The concurrency primitives (`Loader` worker pool with detach-on-drop cancellation; `HlService` with epoch-based stale-result rejection) are correct and stay untouched.

The debt is concentrated in the TUI and traces to three structural roots, each of which also produces a concrete bug. This design records the decisions for paying them down; the analysis behind each is summarized inline.

### Root A — `App` is a god object over hand-synced denormalized state
`App` holds ~40 fields spanning ten concerns (history, the `cs` spine, layout/plan, vertical + horizontal viewport, sidebar windowing, review tracking, two overlays, the loader lifecycle, the highlight service, chrome). The "live" navigation fields are a *copy* of the current `ViewEntry`, synced by hand in `save_current`/`load_current` — and only a subset is round-tripped (`scroll`, `selected`, `viewed`), so `h_scroll` and `wrap` silently leak across view switches. Adding any per-view state means editing four call sites and is easy to get wrong.

A method-by-method trace shows ~40 methods belong to exactly one concern (they migrate cleanly) and ~8 are genuinely cross-cutting (`save`/`load_current`, `drain`/`finish`/`cancel_load`, `open_peek_*`, `open_commit`, `click`). Those 8 all route through the load/view lifecycle — which is `App`'s real, irreducible responsibility.

### Root B — file identity is positional, and one code path mutates the file set
`loader idx`, `viewed[i]`, `selected`, `file_starts[i]`, and the highlight cache are all keyed by position. The file set is immutable for a view's lifetime in every case **except** `resume_load_if_stale`, which re-runs `git::enumerate` for a working-tree view on return. If the working tree changed, the fresh set has a different length/order while `viewed` keeps its old length and `selected` its old index — neither is reconciled. Consequences: misaligned reviewed flags, a stale cursor, and an out-of-bounds **panic** in `next_unviewed` (which indexes `self.viewed[idx]` directly for `idx in 0..cs.files.len()`). Highlights are spared only because `load_current` calls `hl.reset()` and the epoch mechanism discards stale results — the identity guard that `viewed`/`selected` lack.

The same re-enumeration is why an abandoned load loses all progress: the partial `cs` is discarded and `views[cursor].cs` was never updated, so return re-diffs everything.

### Root C — "mode" is implicit and re-derived four ways
There are six input contexts (Peek, Help, Palette, Loading, Normal/Stream, Normal/Sidebar) but no `Mode` type. `handle_key` reconstructs them via an early-return cascade (knows all six); the event-loop mouse arm only special-cases Peek (knows two); `draw_status` derives hints from `palette` + `focus` (knows three); `draw` paints overlays from three independent flags. The gaps are live defects: the status line shows stream hints/context under the peek and help; the mouse leaks through the palette and help; `draw` can stack all three overlays at once because the flags are not mutually exclusive; and the keymap is transcribed three times (the `handle_key` arms, the `draw_status` hint strings, the `HELP_*` consts) with nothing keeping them in sync — the peek is where they have already drifted. `draw_status`'s percentage also divides by `app.plan.rows.len()` (always the stack plan), so it is wrong in split mode.

## Target Architecture

The purpose of this change is the architecture, not the bug fixes (those fall out of it). So the target is stated positively — as a shape to commit to — rather than as "less god object." The shape is **The Elm Architecture (TEA)**: a model, pure update functions over it, and a pure view. It is the right shape for a TUI render loop, and it is also where Rust's borrow checker pushes any decomposition in which a shared cursor (`selected`) is written by several concerns — field-owning components that all need `selected` cannot coexist, so the honest form is *one model + stateless functions over it* (this is why Decision 4 rejects field-owning parts).

```
  EVENT LOOP  (the composition root — holds pieces, contains no state of its own)
    ├── Session   = views, cursor, repo_dir, loader, load flags            ← the one
    │               + lifecycle/nav methods (Decision 3)                     stateful struct
    ├── Mode      = { base: Normal{focus} | Peek(PeekState),               ← interaction state
    │                 overlay: Option<Palette | Help> }   (Decision 6)
    └── hl, theme = services / config (TEA runtime, not model)

  ViewEntry = ViewState + cs + stubs + Plan-cache + kind/req/base          ← per-view model
  Operations: stream / sidebar / review = free fns over (&mut ViewState, &Plan, &cs)
```

There are two kinds of state here, and the split is *ownership*, which decides struct-vs-free-function:
- **Shared model** (`ViewState`, `cs`) — written by several concerns, ownable by none → free-function operation modules over it (Decision 4).
- **Private machine** (`Session`'s loader + load flags + stack) — touched by nothing else, with invariants → an encapsulating struct (Decision 3).

**The organizing invariant is one home per field, on the per-view vs app-global axis.** Every piece of state lands in exactly one cell:

```
                    │ persistent                 │ transient / derived
 ───────────────────┼────────────────────────────┼───────────────────────────
  app-global        │ Session(views, cursor,     │ Mode (base + overlay),
                    │ repo_dir), theme           │ loader+flags (in Session),
                    │                            │ hl service, frame geometry
 ───────────────────┼────────────────────────────┼───────────────────────────
  per-view          │ ViewState, cs, stubs,      │ Plan (cache of cs+viewed+
   (on ViewEntry)   │ kind, req, base            │ layout) — see Decision 5
```

Reading off the grid settles "where does X live" once: `Mode` (with its overlays) is *interaction state* → a loop local, never on `ViewState` or `Session`; `ViewState` + `cs` are *per-view* → owned by `ViewEntry`; `Plan` is *per-view derived* → a cache beside `cs`; the `loader` is *Session's private machine* → inside `Session`; `hl`/`theme` are *services* → loop locals. The operation modules (`stream`, `sidebar`, `review`) own **nothing**; `Session` owns its private machine and nothing shared. Frame geometry is not state at all under pure render (Decision 8) — it is recomputed per frame and owned by no one.

**There is no `App`, and the absence of a root struct is load-bearing (Decision 9).** A named root that holds everything is a gravity well: every new field has an obvious place to land, which is exactly how `App` accreted. The event loop holds its locals and wires them; new state must find a *real* home on the grid because there is no junk drawer to default to.

**Honest caveat — this is not encapsulation.** Operation modules over one shared `ViewState` means any module can touch any field; a reader may call it "a god *struct* with scattered methods." The defense is that the coupling is *intrinsic*: `selected` genuinely is shared cursor state that stream, sidebar, review, and peek all legitimately write, and hiding it behind one owner only forces the others to call through that owner — the indirection that recreates the god object. So the design does not pretend the model is encapsulated; it makes it an honest, well-named `Model` and keeps the functions over it pure. That is the architecture being committed to — not half-encapsulated component structs.

## Goals / Non-Goals

**Goals:**
- Reach the TEA-shaped target above: shared per-view model (`ViewState` + `cs`) acted on by stateless operation modules; the one private state-machine (`Session`) encapsulated; interaction state in a `Mode` of base-plus-overlay; a pure view — every field with exactly one home on the per-view/app-global grid.
- Remove the `App` god object entirely (Decision 9): the load/view machine becomes `Session`, the rest scatters to `Mode`/services/per-frame geometry, and the event loop wires loop locals with no replacement root struct.
- Make a view's file set immutable for its lifetime; preserve completed diffs across switches and resume only the undiffed remainder.
- Fix the `next_unviewed` panic and the reviewed/cursor misalignment by giving files stable identity.
- Introduce a first-class `Mode` that drives keyboard, mouse, status, and overlay selection from one definition; fix the stale status, the mouse leak, and the split-mode percentage.
- Land in independently shippable phases, each behavior-preserving except where it fixes a named defect, guarded by the existing TUI tests plus per-phase regression tests.

**Non-Goals:**
- Changing the diff algorithm, git loading, or highlighting.
- Adding configurable keybindings (the `Mode` + keymap table makes it *possible* later; this change does not ship a config surface).
- Auto-refreshing a working-tree view's file set on return (snapshot semantics deliberately drops the accidental refresh; an explicit reload action is a possible later step).
- Restructuring the non-TUI text-dump path (it stays synchronous and untouched).

## Decisions

### Decision 1: One `ViewState`, owned by the view
Introduce `ViewState { scroll, h_scroll, wrap, selected, reveal_selected, viewed }` and store it on `ViewEntry`. `App` borrows `views[cursor]` instead of keeping live copies; `save_current`/`load_current`'s field-copy logic is deleted. This fixes the `h_scroll`/`wrap` leak (they are now per-view) and makes "add per-view state" a one-line change.

**Sequencing (as built):** the `ViewState` *struct* is the shared substrate the decomposition is built on, so it is introduced in **P3** (when `App` dissolves into `Session` and the borrow structure is reworked) rather than created in P1 and rebuilt in P3. P1 fixes the named leak directly — `h_scroll`/`wrap` join `scroll`/`selected`/`viewed` in the per-view round-trip — so the user-visible defect is gone in the foundation phase while the struct relocation rides with the decomposition.

`selected`/`reveal_selected` live here, not on a `Sidebar`, because they are shared cursor state: the stream writes `selected` while scrolling (`anchor_selected`), the sidebar writes it on move/click, review writes it on next-unviewed, the peek reads it. `reveal_selected` is the stream→sidebar "scroll to show the cursor" signal. Making them view state — and the stream/sidebar *views onto it* — is what lets the parts (Decision 4) stay non-aliasing.

### Decision 2: Snapshot file identity — the view owns its stubs
`ViewEntry` owns the enumeration stubs (e.g. `Arc<Vec<FileStub>>`) alongside its `cs`. A view's `cs.files` length and order are fixed at creation and never change. Consequences:
- **Resume** = for each `i` where `!cs.files[i].diffed`, re-diff `stubs[i]` at its original index. Indices are stable, so no reconciliation is needed and completed diffs are kept.
- **No re-enumeration** on return; `resume_load_if_stale` re-runs only the leftover diffs, hitting git for nothing it already has.
- **The panic is impossible by construction**: `viewed.len()` always equals `cs.files.len()` for the view's lifetime.

The cost is giving up the accidental working-tree refresh-on-return, which never worked correctly anyway (it was the source of the panic). A deliberate `reload` action can reintroduce it later with proper path-keyed reconciliation.

**Alternative considered — live + reconcile:** keep re-enumerating but carry `viewed`/`selected` across the reshuffle by path. Rejected for this change: it is strictly more code than snapshot, reintroduces the "file list changes under you" surprise, and snapshot already delivers the resumable-load win. Reconcile is the right tool only for an *explicit* reload, not an implicit one.

### Decision 3: `Session` owns the load/view state-machine (and `App` dissolves)
The cross-cutting orchestration — `begin`/`drain`/`finish`/`cancel`/abandon/resume and the view-switch glue (`push_view`, `view_back`/`forward`/`home`, `open_commit`) — is **not** spread across loop locals; it is a state machine with *private* state (the `loader` worker pool, `load_started`, `load_is_switch`) and invariants (a loader exists ⟺ a load is in flight for `cursor`; abandoning must detach cleanly; resuming re-dispatches only undiffed stubs). Private state with invariants and transitions is exactly what a struct with methods is for — so this lives in one type, **`Session`**, which owns `views`, `cursor`, `repo_dir`, the `loader`, and the load flags, and exposes the lifecycle/nav methods.

**Why a struct here, when Decision 4 makes the other operation groups free functions:** the difference is *ownership of state*. `stream`/`sidebar`/`review` are free functions because their state (`ViewState`, `cs`) is **shared** — no one can own it, so the borrow checker forces functions-over-an-external-model. The lifecycle's state is the opposite: **private** — nothing outside touches the worker pool or the in-flight flags. Decision 4's "free functions" rule is specifically the *shared-state* case; it does not generalize to private state. Encapsulating the one genuinely stateful machine behind a boundary that can enforce its invariants is the right tool for the highest-churn, most-bug-prone code (all four named defects lived here).

**`Session` is cohesive, not a relabelled god object.** The cohesion test: every field participates in the one concern *browse-and-load views* — `views`, `cursor`, `repo_dir`, `loader`, `load_started`, `load_is_switch` all do. The things that made `App` a god object — `Mode`, the overlays, `hl`, `theme`, render geometry — explicitly stay **out** of `Session` (they are interaction state / services / per-frame derived; see Decision 9). `App` itself ceases to exist: there is no god root that holds everything. (The name `History` was considered for the stack alone; once the type also owns the in-flight load, `Session` is the honest name and `History` = its `views`+`cursor` sub-part.)

**The `make_mut` story inverts under Decision 2 — and that is the load-bearing subtlety of P1.** Today there are *two* `Rc<Changeset>` handles to the same allocation: `App.cs` (the live spine) and `views[cursor].cs` (the stored entry), because `load_current` does `self.cs = e.cs.clone()`. On the first drain, `Rc::make_mut` sees a refcount of 2 and *clones to detach* `self.cs` into a fresh allocation; subsequent drains are in place. That deliberate detach is exactly why abandoning a load loses progress — the loader writes into the detached `self.cs` while `views[cursor].cs` keeps pointing at the stale stub allocation, so a later `load_current` clones the stale entry back. The "first drain clones once" optimization and the "abandon loses work" bug are *the same mechanism*.

The end state (P3) unifies the two handles into one: there is no independent live `cs` clone; the live spine **is** `views[cursor].cs` (reached through `Session`), and the loader installs into it directly. With a single handle the refcount is 1, so `make_mut` never clones — every drain is in place for free. That collapse rides on the `Session` accessor boundary, so it lands when `App` dissolves (P3), not before.

**As built (P1):** the divergence is closed at the *choke point* instead, which is the smallest correct diff and does not require touching every `self.cs` read. `App` keeps its live `cs` handle, and the live `cs` is synced back onto the entry in `save_current` — the single function every view switch passes through — and on `finish_load`. So the entry always holds the latest partial before it can matter (before we leave the view, or on completion), and a resumed load re-diffs only the still-undiffed stubs at their original index. This retains the one pre-existing detach-clone per load (the first `make_mut`), which is O(1) on the changeset spine and unrelated to correctness. Crucially this is **not** the half-migrated trap — diffs are installed into exactly one place (`self.cs`) and the entry is overwritten wholesale at the choke point, so the two never diverge; the trap is installing *into the entry* while a separate live clone drifts.

The one remaining constraint is render-side transient clones: `request_visible` clones the live `cs` (refcount → 2) to hand to the highlighter and drops it within one event-loop statement. As long as those stay transient, a drain never overlaps a live clone and never falls back to copy-on-write. P1's `transient_cs_clone_does_not_corrupt_install` test pins the copy-on-write correctness; the resume-only-remainder and progress-retention tests pin the choke-point behavior.

### Decision 4: `stream` / `sidebar` / `review` / `history` as stateless operation modules
Extract the cohesive operations from `App` as **modules of free functions, not zero-field structs**. A `struct Stream;` whose methods all take `&mut ViewState` is encapsulation cosplay — an object that owns nothing; the honest form is `mod stream { pub fn scroll_by(st: &mut ViewState, plan: &Plan, …) }`. To avoid borrow-checker fights with the shared `cs` + `ViewState`, the modules **own no per-view data**; they take `(&mut ViewState, &Plan, &cs)` (or the minimal slice they need). This keeps each trivially unit-testable without constructing an `App`, and it *is* the TEA update layer from the Target Architecture.

`history` is the one operation grouping that legitimately owns persistent app-global state (`views`/`cursor`/`repo_dir`); it may stay a struct because it has fields to own. The derived `Plan` is **not** owned by `stream` — it is a per-view cache on `ViewEntry` (Decision 5); `stream` rebuilds and reads it but does not hold it. The overlays (`Palette`, `Peek`) are app-global UI state and stay structs.

**Alternative — parts own disjoint fields:** rejected. The cursor (`selected`) and the `cs` spine are shared, so field-owning structs would force `stream::scroll_by` to take `&mut Sidebar`, recreating the god object with extra indirection.

### Decision 5: One parametric `Plan`
Collapse `Plan`/`SplitPlan` and `Row`/`SRow` into a single `Plan { rows, file_starts, hunk_starts, change_starts, content_w, layout }` with `Row::Body(Cell, Option<Cell>)` (stack = one cell, split = two) and the five chrome variants written once. `Plan::build(cs, viewed, layout)` branches only in the hunk-body emission (interleave vs the split `flush` pairing).

**`Plan` is per-view derived state, so it is cached on `ViewEntry`** (the bottom-right cell of the taxonomy), not on `App` and not on `stream`. Its inputs are all per-view (`cs`, `viewed`, `layout`), so its home is beside them; it is rebuilt when any input changes — a layout toggle, a streamed batch (`viewed`/`cs` grew), or a review mark. Caching one `Plan` per view (instead of today's `App`-held `plan` + `split` pair) deletes the `if split_active { &self.split.x } else { &self.plan.x }` accessor triplets *and* keeps each view's plan warm across switches rather than rebuilding on every return. `stream` rebuilds and reads this cache but owns no part of it, preserving "operation modules own nothing."

The two plans genuinely differ in row count and ordering (a 3-removed/1-added run is 4 stack rows but 3 split rows), and navigation is index-based over the active plan — so a single shared row list serving both layouts is impossible; building per-layout is the honest model. The leaf renderers stay two paths (stack uses `Paragraph` + optional wrap; split uses `clamp_pad` columns + the `│`) co-located under one dispatch. This is a prerequisite for Decision 7.

### Decision 6: First-class `Mode` — a base with at most one overlay
A flat `enum Mode { Normal, Palette, Peek, Help }` is wrong: it makes `Help` a *sibling* of `Normal`/`Peek`, so it **forgets what it is overlaying** — help cannot render mode-appropriate keys (stream keys over `Normal`, peek keys over `Peek`) and cannot know where to return on close. `Help` and `Palette` are not modes you *live in*; they are transient layers you *summon over* a base, that capture input and dismiss back to it. So `Mode` has two axes:

```
enum Base    { Normal { focus: Focus }, Peek(PeekState) }  // what you're looking at; you live here
enum Overlay { Palette(Palette), Help }                    // summoned on top; remembers its base
struct Mode  { base: Base, overlay: Option<Overlay> }      // at most one layer, never forgets the base
// Loading stays an orthogonal flag on Session, composable with any layer.
```

This makes `Mode` the single source of truth while fixing the modeling flaw:
- **Input routing, one precedence:** `if let Some(ov) = overlay { route to ov } else { route to base by focus }`. Keyboard and mouse share it; overlays capture the wheel/click, ending the leak-through.
- **Help depends on its base:** `help_for(mode.base)` — help over `Normal` lists stream keys, help over `Peek` lists peek keys; closing restores the retained base. This is the relationship the flat enum destroyed.
- **Exactly one overlay is a *type* guarantee, not a discipline:** `Option<Overlay>` replaces the three independent `bool`/`Option` flags, so overlays cannot stack or disagree by construction — and, unlike the flat enum, they no longer lose their base.
- **Status line** reads the active layer: an overlay yields its own hint/context (Peek base → the peek's label + its scroll %; `Normal` → file # + the **active plan's** scroll %, fixing the split-mode bug); the keymap table is keyed by the active `Base`/`Overlay`, and both the status hints and the `?` help render from it, so the three transcriptions collapse to one and cannot drift.

**Overlays are pure widgets that return a result; the loop dispatches it.** This matters most for `Palette`, the one overlay whose action crosses the `Session` boundary. `Palette` is a reusable fuzzy-finder with two backings (`PaletteKind`): **Files** (filter the current view's `cs.files`; confirm → jump the cursor) and **Commits** (a commit/history picker — range-aware, optionally file-scoped via `F`; confirm → open the chosen commit as a new view). Rather than the overlay reaching into the model on confirm, it emits a result — `Jump(idx)` or `OpenCommit(rev, scope)` — and the event loop routes it: `Jump` → `ViewState.selected` (in-view), `OpenCommit` → `Session::open_commit` (lifecycle). The overlay stays interaction-state-only, and the `Session` boundary stays clean. (Its transient `Vec<CommitInfo>` is just the list it enumerated to pick from — not part of the view stack, so `Palette` does not belong inside `Session`.)

This is the expanded scope of what was originally "data-driven keybindings": the value is the bug fixes (stale status, mouse leak, wrong %, can't-stack) *plus* a faithful interaction model, with config-driven keys as a free downstream possibility.

**Sequencing (as built):** the `Mode` *type* is interaction state the loop owns (Decision 9) and the keymap-table rewrite reworks routing, so both land in **P3** with the decomposition — introducing `Mode` as an `App` field in an early P2 only to relocate it in P3 would churn ~80 call sites and the tests twice. P2 instead ships the three user-visible defects directly against the current flags (the smallest correct diff): the status line reflects the active overlay (peek/help show their own context, not the stream's), the percentage uses the on-screen layout's row count (`scroll_pct(scroll, active_rows)`), and the loop absorbs the mouse while the palette/help overlay is open. The keymap-drift fix and the base+overlay type then arrive whole in P3.

### Decision 7: Peek as a `Stream`
Once Decisions 4–6 land, the peek is "the `stream` operation module over a synthetic one-file `Changeset`, carried as `Base::Peek(PeekState)`." `PeekState` is a self-contained mini-view (`cs` + `ViewState` + `Plan`-cache) — a modal view that lives *in the `Mode`*, not in the `Session` stack (it is not navigable history). Its parallel `peek_*` navigation, its dual `plan`/`split` + `change_starts`/`split_change_starts`, and its `active_rows`/`active_change_starts` is-split switching all collapse into the shared `stream` functions + the single parametric `Plan`. The peek-open path (sourcing the file's text from git for a stub file) stays `Session` logic, since it reads the view's source.

### Decision 8: Pure render
Split `draw` into a measure pass (returns viewport/sidebar geometry) and a paint pass that is a pure function of `(&Mode, &Session, services)`. Geometry feeds back through the event loop, which already drives highlight requests via `request_visible`. With `Mode` selecting the overlay (by `Base` + `Option<Overlay>`) and carrying its context, `draw` stops needing ad-hoc `if flag` checks and mutates nothing.

### Decision 9: `App` dissolves into `Session` + loop locals — no root struct
`App` does not shrink into a "thin coordinator"; it is **removed**. Its responsibilities scatter to their real homes: the load/view machine → `Session` (Decision 3); interaction state → `Mode` (Decision 6); highlighting + theme → services; render geometry → per-frame, owned by no one (Decision 8). What remains at the top is the **event loop**, which owns these as locals and wires them — `poll event → route by Mode → mutate Session/ViewState via the operation modules → drain loader → measure → paint`. The loop is a skeleton: it accretes no state and holds no business logic (routing is the `Mode`-keyed table; per-key logic is in the operation modules).

**Why no replacement aggregate.** A `Tui { session, mode, hl, theme }` struct "just to pass around" would reinstate the gravity well one ring out — the next field would land in `Tui` exactly as it once landed in `App`. The discipline that keeps the decomposition from re-collapsing is *structural*: there is no single struct that holds everything, so each new field is forced onto the grid. `Session` is allowed to be a struct because it is **cohesive** (one concern, every field in its private machine); a top-level bag would be the opposite.

**Test ergonomics.** The existing TUI suite drives `App` and asserts on its fields; that is the safety net (a stated risk), so the migration must keep it drivable. Post-dissolution: navigation/review/streaming tests drive a `Session` directly (their domain), mode/status/overlay tests drive a `Mode`, and full-flow tests use a small `harness()` helper that constructs the few pieces and runs one loop turn. The helper is a *test* convenience that returns the pieces — not a production type — so it does not become the new root.

## Sequencing

```
 P1  ViewState + snapshot identity     foundation; fixes next_unviewed panic; enables resume
 P2  Mode (base+overlay) + keymap       independent; fixes status / mouse / split-%; can land early
 P3  dissolve App → Session + modules   depends on P1 (ViewState is the shared substrate)
 P4  one parametric Plan                collapses the stack|split axis
 P5  Peek = stream over 1-file cs       depends on P3 + P4
 P6  pure render(Mode, Session)         depends on P2 (Mode-driven overlay) + P4
```
P1 and P2 are the high-value entry points (each fixes named defects). P2 is dependency-independent and can ship first if a quick visible win is wanted. P3 is where `App` actually dissolves (`Session` extracted, operation modules carved out, the root removed); P3→P5 are the structural spine that removes the duplication.

## Risks / Trade-offs

- **Largest refactor of the most-tested code.** Mitigation: the existing TUI suite is the regression net; each phase adds a focused test (panic repro, resume-only-undiffed count, status-reflects-mode, mouse-does-not-leak, layout-toggle re-anchor).
- **`make_mut` uniqueness under view-owned `cs` (Decision 2/3).** This is P1's highest-stakes detail; see Decision 3 for the full inversion. In short: unify the live `cs` with `views[cursor].cs` into one handle (never two), after which `make_mut` never clones and progress persists automatically. The only failure mode is a long-lived render clone overlapping a drain; today's `request_visible` clones drop within one statement — keep them so, and assert it with a test that drains while a render-measure has run. **De-risk this first:** P1 is really two sub-steps — (a) unify the `cs` handle + install-into-entry + resume, and (b) relocate the navigation fields into `ViewState`. Land (a) before (b) so the `make_mut`/resume behavior is proven on the smallest possible diff before the state-relocation churn lands on top.
- **`App` removal is a P3 cutover, not a rename (Decision 9).** The risk is re-collapse: a `Tui`/`Session`-as-everything bag would defeat the purpose. Mitigation: enforce the grid in review — `Session` holds only its private machine, `Mode` only interaction state, services are loop locals, and there is no struct that holds all three. The existing `App`-driven tests migrate to driving `Session`/`Mode`/the `harness()` helper; keep them green through the cutover as the net.
- **Layout toggle now rebuilds the plan (Decision 5)** instead of flipping a pre-built bool. Rebuild is already O(rows) per stream batch, so the cost is negligible, but `cycle_mode` must preserve the current-file anchor (a test guards it).
- **Snapshot drops working-tree refresh-on-return (Decision 2).** Accept as a deliberate behavior change (the old behavior was buggy); document, and leave an explicit reload for later.
- **Phase independence.** P2 must not assume P1; P5/P6 must not be attempted before their deps. Keep the phase boundaries as the proposal/tasks define them so each PR is reviewable in isolation.

## Resolved Questions

- **`ViewState` access style → `&mut ViewState` per call.** The alternative (borrow the active `ViewState` for a part's lifetime) reads nicer but loses to the borrow checker the moment a part needs `&mut ViewState` and `&cs` together — which `Stream::rebuild` does. Per-call passing is also what keeps the parts unit-testable without an `App`. Settled; do not relitigate at P3.
- **`Loading` is an orthogonal flag, not a `Mode` variant.** The flag keeps the cancel hint composable with any mode. The "revisit if a load can coexist with the peek" caveat is answerable now and is a *no*: the peek opens a synthetic one-file changeset that is already diffed (Decision 7), so the peek never coexists with a *stream* load. The flag is safe.
- **The keymap is a static data table, not a shared `match`.** P2's entire value is that hints, help, and routing cannot drift. A `match` that the help renderer also calls still lets the help *text strings* drift from the match *arms* — only data the help renderer reads makes drift structurally impossible. The static table also leaves configurable keys as a free downstream step. Decided toward data despite the extra machinery.
