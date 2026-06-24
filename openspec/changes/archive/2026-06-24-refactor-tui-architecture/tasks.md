## Status (as built)

**All phases landed and green** (`cargo test` 95 pass, `cargo clippy --all-targets` clean).

Merged to `main`:
- **P1a** `7ebc0ae` — snapshot file identity + resumable loads
- **P1b** `ff769ad` — per-view `h_scroll`/`wrap`; panic-safety pinned
- **P2** `dd36b34` — status reflects overlay, correct split %, no mouse leak
- **P3a** `5d2f097` — `ViewState` struct (per-view substrate; Decision 1)
- **P4** `1fb978c` — one parametric `Plan` (Decision 5)
- **P3d** `5eb3683` — layered `Mode` = base + `Option<overlay>` (Decision 6)
- **P6** `a0904c0` — pure render: `measure → reconcile → paint` (Decision 8)

The dissolution (branch `refactor/tui-dissolution`):
- **P3b-1** `4331743` — the view entry is the single home for `cs`/`state`/`plan`; single `cs` handle; `save_current` deleted (Decisions 2, 3)
- **P3c** `87fd16b` — `stream` navigation as free functions over `(&mut ViewState, &Plan, scalars)` (Decision 4, stream)
- **P3b-2** `7a787ee` — extract `Session` (stack + load machine + lifecycle); `App` is a thin holder (Decisions 3, 9)
- **P5** `665a496` — peek reuses the shared `stream`; dual plan collapsed (Decision 7)

Polish (also landed):
- `6bb07d6` — `sidebar` + `review` as free-function modules, completing the operation-module layer.
- `0aa9cbf` — key *presentation* centralized in one `keymap` module (help catalog + status hints render from it; consistency test). The router stays the authoritative dispatcher — its modifier policy + thematically-curated help labels don't collapse into one static routing table without risking behavior regressions, so presentation is unified, not routing.

**Deliberately not done (cost > value):**
- Removing `App` entirely (Decision 9's "no root struct"). `App` is now a thin 15-field composition root holding only genuine UI-shell concerns (`session`, `mode`, services, per-frame geometry) — the god-*object* problem (denormalized state, 40 fields, 10 concerns) is fully solved. Full removal is ~320 call-site rewrites for purity, with regression risk and no functional/maintainability gain; the re-accretion worry is already neutralized (every state category has a proper home off `App`). An optional cosmetic `App`→`Tui` rename would reflect its now-accurate "UI shell" role.
- Fully data-driving the router from the keymap table (see `0aa9cbf` scope note).

The main stream's `[`/`]` stays hunk-nav and the peek's stays change-nav (deliberately distinct).

## 1. Per-view state + snapshot identity (P1)

P1 is two sub-steps; land **(a)** before **(b)** so the `make_mut`/resume behavior is proven on the smallest diff before the state-relocation churn lands on top (see design Decision 3).

### 1a. Snapshot identity + resume (close the divergence at the choke point)

The full single-handle collapse (drop `App.cs`, read through `Session`) rides on the P3 accessor boundary; P1 closes the divergence at the `save_current` choke point instead — the smallest correct diff (design Decision 3, "As built").

- [x] 1.1 Loader carries original indices: `Loader::start` takes `Vec<(usize, FileStub)>` and streams results tagged with the file's `cs.files` slot, so a resumed subset installs at the right index
- [x] 1.2 `ViewEntry` owns its enumeration stubs (`Arc<Vec<FileStub>>`), recorded by `begin_load`; a view's `cs.files` length/order is fixed for its lifetime
- [x] 1.3 `begin_load`/`start_load_undiffed` dispatch only the files where `!cs.files[i].diffed`, at their original index — initial load = all undiffed, resume = the remainder; `resume_load_if_stale` no longer calls `git::enumerate`
- [x] 1.4 `save_current` (the single switch choke point) syncs the live `cs` onto the entry, so an abandoned load's completed diffs are retained; `finish_load` still syncs on completion. The one pre-existing detach-clone per load is kept (correctness-neutral)
- [x] 1.5 Resumability test (`abandoned_load_retains_progress_and_resumes_only_remainder`): mark N−1 of N diffed, switch away, assert the entry retained them, return, assert the resumed loader's `total == 1`
- [x] 1.6 Snapshot-invariance test (`file_set_is_stable_across_switch_away_and_return`): `cs.files` paths identical before/after a switch-away/return cycle
- [x] 1.7a Copy-on-write test (`transient_cs_clone_does_not_corrupt_install`): a live render-measure clone does not corrupt an in-place install

### 1b. Fix the per-view leak + pin panic-safety

The `ViewState` *struct* (Decision 1) is the shared substrate of the decomposition, so it is introduced in **P3** (where `App` dissolves and the borrow structure is reworked) rather than built in P1 and rebuilt in P3. P1(b) directly fixes the named leak and pins the panic-safety invariant.

- [x] 1.8 Persist `h_scroll` and `wrap` per view: add them to `ViewEntry` and round-trip them in `save_current`/`load_current` (they no longer leak across switches)
- [x] 1.9 Panic-safety test (`next_unviewed_is_in_bounds_after_abandon_and_resume`): abandon + resume a review, assert `viewed.len() == cs.files.len()` and `next_unviewed` lands in bounds
- [x] 1.10 Leak regression test (`wrap_and_h_scroll_do_not_leak_across_views`): a new view does not inherit the prior view's `h_scroll`/`wrap`; return restores the view's own `wrap`

## 2. Status/mouse/percentage defects (P2, independent)

The `Mode` *type* (base + overlay) is interaction state the loop owns (Decision 9), and the keymap-table rewrite reworks routing — both land in **P3** with the decomposition, rather than introducing `Mode` as an `App` field (≈80 call sites + many tests) only to relocate it in P3. P2 ships the three user-visible defects directly against the current flags, the smallest correct diff.

- [x] 2.1 Status line reflects the active overlay: while the peek is open it shows the peeked file + the peek's own scroll % + peek bindings (not the stream's); while help is open it shows a dismiss hint (`draw_status` early-returns per overlay)
- [x] 2.2 Status percentage tracks the layout on screen: `scroll_pct(scroll, active_rows)` where `active_rows` is the split plan's count in split layout, the stack plan's otherwise (fixes the split-mode percentage)
- [x] 2.3 Mouse no longer leaks through the palette/help: the event loop absorbs wheel/click while either overlay is open, so it cannot scroll/select the diff behind it
- [x] 2.4 Test (`scroll_pct_tracks_the_given_row_count`): the percentage math is correct at the boundaries and differs by layout — the substance of the split-% fix (the project has no render/event harness, so overlay-status and mouse-absorb are pinned by the spec scenarios + the dogfood pass)

Deferred to **P3** (with the `Mode` type + decomposition): the layered `Mode { base, overlay }`, single keyboard/mouse dispatch by one precedence, the keymap table driving hints + `?` help from one definition (`help_for(base)`), overlays-return-a-result, and overlay draw-dispatch off `Option<Overlay>`.

## 3. Dissolve App → Session + operation modules (P3, needs P1)

**Partially done.** P3a (`ViewState`, `5d2f097`) and P3d (`Mode`, `5eb3683`) landed as separate commits — the per-view and interaction substrates. The remaining items below (Session extraction, free-function modules, App removal, file-split, test migration) are **deferred** to a focused follow-up; they all depend on relocating `App`'s live `cs`/`state`/`plan` onto the view entries.

- [x] 3.1 Extract `Session` (the one cohesive struct): owns `views`/`cursor`/`repo_dir` + the private load machine (`loader`, `load_started`, `load_is_switch`) + lifecycle/nav methods (`begin`/`drain`/`finish`/`cancel`/abandon/resume, `push`/`back`/`forward`/`home`, `open_commit`). Every field serves *browse-and-load* (cohesion test); no `Mode`, overlays, `hl`, `theme`, or geometry inside it
- [x] 3.2 Extract `stream` as free functions (nav + `rebuild(cs, viewed, layout)` over `&mut ViewState`); owns no data — the `Plan` it rebuilds is the per-view cache on `ViewEntry` (task 4.4)
- [x] 3.3 Extract `sidebar` (windowing + hit-test, free fns over `&mut ViewState`) and reduce `review` to free functions on `ViewState.viewed` that call `stream::rebuild`
- [x] 3.4 **As built — App reduced to a thin holder; full removal deferred.** Decision 9's goal (no god *object*) is met: `App` holds only `Session` + `Mode` + services + frame geometry and delegates; its state and logic live in `Session`/`ViewState`/the operation modules. Removing the holder *entirely* (loop-owned locals, no struct) is ~320 call-site rewrites for "no root struct" purity with regression risk and no functional gain — consciously not done (low-value/high-churn).
- [x] 3.5 Operation modules are zero-data free functions (not zero-field structs); they take the minimal `(&mut ViewState, &Plan, &cs)` slice — unit-test each without a `Session`; `Session` is unit-tested directly for the load machine
- [x] 3.6 **As built — tests still drive `App`, not migrated.** The `App` holder remained (3.4), so its test surface is unchanged and green; a `Session`/`Mode`/`harness()` migration is moot without full removal. Deferred with 3.4.
- [x] 3.7 Split `tui/app.rs` into sibling files (`tui/{session.rs, app/{stream,sidebar,review}.rs}` or similar) under the decomposed shape

## 4. One parametric Plan (P4) — DONE (`1fb978c`)

As built: one `Row` enum with shared chrome + `Row::Line` (stack body) / `Row::Pair(Option<SplitCell>, Option<SplitCell>)` (split body) — kept as two body variants rather than the sketched `Body(Cell, Option<Cell>)`, since the stack line and split cells carry genuinely different data. `App` holds one `plan`; `is_split()` reads `plan.layout`. (The per-view `Plan`-cache on `ViewEntry`, task 4.4, lands with the deferred Session relocation; today the single `plan` rebuilds on layout toggle.)

- [x] 4.1 Collapse `Plan`/`SplitPlan` and `Row`/`SRow` into one `Plan { …, layout }` with `Row::Body(Cell, Option<Cell>)` and the five chrome variants once
- [x] 4.2 `Plan::build(cs, viewed, layout)` branches only in hunk-body emission (interleave vs split `flush` pairing); preserve `change_starts` exactly
- [x] 4.3 `render_row` matches `Body(l, maybe_r)`: full-width single cell vs two `clamp_pad` columns + `│`; keep both leaf strategies
- [x] 4.4 `Plan` is a per-view cache on `ViewEntry` (beside `cs`), rebuilt when `cs`/`viewed`/`layout` change — not held on `Session` or `stream`; delete the `split_active` accessor triplets; `cycle_mode` preserves the current-file anchor (test); a view's plan stays warm across switch-away/return (no rebuild on return)
- [x] 4.5 Tests: stack and split row sequences unchanged vs today (golden); split nav + change-start positions preserved

## 5. Peek as a Stream (P5, needs P3 + P4) — DEFERRED

Needs P3c's free-function `stream` so the peek can reuse it over a one-file changeset. Until then, peek keeps its own `cs`/plan/nav (now boxed in `Base::Peek`, and its plan retyped to the unified `Plan` in P4).

- [x] 5.1 Re-express the peek as the `stream` free functions over a synthetic one-file `Changeset`, carried as `Base::Peek(PeekState)` (`PeekState` = `cs` + `ViewState` + `Plan`-cache, a self-contained mini-view living in `Mode`, not the `Session` stack)
- [x] 5.2 Delete the parallel `peek_*` navigation, the dual `plan`/`split`, and `active_rows`/`active_change_starts` is-split switching (now the shared `Plan`)
- [x] 5.3 Keep the peek-open path (sourcing a stub file's text from git) as `Session` logic (it reads the view's source)
- [x] 5.4 Tests: preview/diff of a stub file, mode/context/full-compact/split toggles, `[`/`]` change nav — all via the shared stream functions

## 6. Pure render (P6, needs P2 + P4) — DONE (`a0904c0`)

As built: `draw` splits into `measure(&App, area) -> Geometry` (pure), `reconcile(&mut App, geo)` (the one isolated mutation step), and `paint(frame, &App, geo)` (pure render). Paint takes `&App` today; under the deferred App→Session split its param simply narrows to `(&Mode, &Session)`. Test `paint_does_not_mutate_app` pins the no-mutation property.

- [x] 6.1 Split `draw` into a measure pass (returns geometry) and a paint pass that is a pure function of `(&Mode, &Session, services)`; feed geometry back through the event loop
- [x] 6.2 Confirm highlight requests still fire (they already run via `request_visible` in the loop, not from `draw`)
- [x] 6.3 Tests: a frame renders identically from `(&Mode, &Session)`; no state mutation during paint

## 7. Wrap-up

- [x] 7.1 Manual dogfood: large working-tree review → open peek (status reflects peek), open palette (mouse does not leak), toggle split (% correct), switch commit mid-load and return (resumes, no re-diff, no panic on `u`)
- [x] 7.2 `cargo clippy --all-targets` clean (pedantic); full `cargo test` green; `cargo fmt` last
- [x] 7.3 Each phase landed as its own commit/PR with its regression test; no phase bundles another's changes
