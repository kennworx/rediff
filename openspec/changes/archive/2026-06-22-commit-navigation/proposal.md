## Why

rsdiff today shows exactly one diff for its whole session: whatever you launched with. To understand a change you almost always need its history — what commit introduced a line, how a file evolved, what a branch's commits did. Today that means quitting and relaunching with a different `show`/range. This change turns rsdiff from a single-diff viewer into a history browser while staying strictly viewer-only (no checkout, no staging): pick any commit from inside the TUI, dive into a file's history, step back and forth like a browser, and review a commit or a whole branch range with the same viewed-tracking you already have for local changes.

## What Changes

- **In-TUI commit picker (`c`)** — a filtered popup listing recent commits (cap 200) with number shortcuts, mirroring the existing fuzzy file palette. Selecting a commit switches the view to that commit's changes (`show` semantics: commit vs its parent).
- **Smart picker filter** — the query is interpreted: a hex prefix filters by SHA; a string that matches a repo path scopes the list to commits that touched that path; anything else fuzzy-matches the commit summary.
- **File-scoped commit log (`F`)** — open the picker scoped to the *selected* file's history (commits that changed it), from any focus.
- **Browser-style view history** — a view stack with `{` (back) and `}` (forward), each view remembering its own scroll/selection. `C` jumps back to the launch ("home") view; it is inert when rsdiff was launched on a commit.
- **Commit / range review** — new `rsdiff review [sha] [--from <base>]` command. With no args it reviews HEAD; `--from <base>` reviews the range as a single **combined net diff** (merge-base of base and target → target), the GitHub "Files changed" model. Review views carry viewed-tracking; the picker is scoped to the range's commits.
- **Per-view review sessions** — viewed-tracking (`v`/`u`/`✓`/collapse) becomes a property of a *view*, not a global. The launch view (working tree, staged, or `review`) is a review session; commits reached by browsing are not. Browsing into history never disturbs review progress, and returning restores it exactly. An in-TUI `R` promotes the current browse view into a review session.
- **Source color coding** — the status line and sidebar file markers are tinted by the current view's source: blue for local changes, green for a commit.
- **Async highlight on switch** — switching a view drops the highlight cache and bumps an epoch so a late result from the previous view cannot paint the new one; visible files re-highlight in the background.

## Capabilities

### New Capabilities
- `commit-navigation`: the in-TUI commit picker and its smart filter, file-scoped commit log, the browser-style view-history stack (`{`/`}`/`C`), source color coding, and the highlight-cache reset on view switch.

### Modified Capabilities
- `changeset-loading`: add commit enumeration (rev-walk from a tip, capped), file-scoped commit history (commits that touched a path), and the `review` command loading a commit or a range as a combined net diff via merge-base.
- `viewed-tracking`: reviewed state becomes per-view (a review session) rather than a single global, so a commit or range can be reviewed and review progress survives browsing into history.

## Impact

- **CLI** (`src/cli.rs`): new `review` subcommand (`[sha] [--from <base>]`); resolves to a new load request.
- **Git** (`src/git.rs`): commit enumeration, path-touched-by-commit checks, merge-base + range net diff, a `LoadRequest::Review`/range variant.
- **TUI** (`src/tui/`): `App` stops borrowing a single `&Changeset` and owns a **view stack** of `Rc<Changeset>` entries, each with its own scroll/selection and optional `viewed`; `run()` is threaded a repo handle so it can load live; the palette generalizes to a commit-picker mode; new keys `c`/`F`/`C`/`R`/`{`/`}`; `v`/`u`/`✓` gate on the current view being a review session; status line and sidebar markers gain source color; `HlService` gains a clear-and-epoch on switch.
- **No new dependencies** — gix already provides rev-walk and merge-base; ratatui/imara-diff unchanged. Speed budget unaffected (rev-walk + a screenful of summaries is sub-ms; range net diff reuses the existing tree-to-tree path).
