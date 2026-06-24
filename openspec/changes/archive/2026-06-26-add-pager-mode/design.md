## Context

rediff is a TUI diff viewer (`gix` + `imara-diff` + tree-sitter/syntect + ratatui). In lazygit, the inline diff panel is fed by an external tool. The user currently wires difftastic via `externalDiffCommand: difft --color=always`. Two problems: difft's coloring is sparse, and `externalDiffCommand` is **display-only** — lazygit reverts to `git diff --no-ext-diff` the instant you focus a file to stage, so the external render vanishes exactly when you interact (lazygit #4724).

lazygit has three diff-display integration points:
- **native** — git's own colored unified diff;
- **pager** (`git.pagers[].pager`) — post-processes git's real patch for display only; the stageable patch survives underneath (how `delta` coexists with staging);
- **externalDiffCommand** — replaces git's diff entirely via `GIT_EXTERNAL_DIFF`; output is not a patch, so staging cannot use it.

rediff is architecturally a **line** differ (`imara-diff`) with tree-sitter *highlighting* — i.e. delta-shaped, not difftastic-shaped. That makes the **pager** point the natural fit: git produces the patch, rediff repaints it, staging keeps working. The existing codebase already has the rendering primitives (`highlight::{Span, Paint, FileHighlight, Highlight}`, theme resolution) and a headless pipe path (`render::to_unified_string`, used today when stdout is not a TTY) — but that path emits **plain** text and computes the diff from the repo. The missing pieces are: a stdin patch parser, and a *colored* headless renderer.

## Goals / Non-Goals

**Goals:**
- A non-interactive `rediff pager` that reads a unified diff on stdin and writes rediff-themed, syntax-highlighted ANSI to stdout.
- Preserve lazygit line/hunk staging (be a pager, not an external differ).
- One source of visual truth: the inline glance and the `<c-g>` full-screen view render in the same theme/colors.
- Reuse the existing highlight engines and theme resolution; add only an adapter + ANSI line renderer.

**Non-Goals:**
- No interactivity, navigation, or viewed-tracking in pager mode (that stays in the TUI under `<c-g>`).
- Not replacing the plain `to_unified_string` pipe fallback for the existing subcommands.
- Not decoding binary patch *content* (only detecting binary and showing a notice).
- Not wiring `diff.external` globally — the user deliberately keeps bare `git diff` plain for other tooling.
- `rediff print` (external/full-file mode) is explicitly deferred to phase 2.

## Decisions

### Decision: Be a pager, not an externalDiffCommand
Wire as `git.pagers[].pager: rediff pager`, replacing the difft `externalDiffCommand`.
- **Why:** the pager point post-processes git's patch, so the unified diff survives for `git apply`-based staging; external mode discards it and reverts on focus.
- **Alternative — keep external (`rediff print` inline):** rejected for lazygit — same focus-revert as difft, plus it would force per-file blob re-diffing. It remains useful for the *terminal* (`git diff | …`), hence phase 2, not here.
- **Alternative — theme `delta` to match:** zero code, but two theme engines (delta's syntect/Sublime vs rediff's tree-sitter+theme) drift; loses the single-source-of-truth goal.

### Decision: Parse with `diffy` 0.5 (`patch_set` + `ParseOptions::gitdiff()`)
rediff has no patch parser (it computes diffs). `diffy`'s `patch_set` layer gives exactly the model needed, verified against published 0.5.0 source:
- `PatchSet::parse(input, ParseOptions::gitdiff())` — streaming, multi-file, tolerant of git extended headers.
- `FilePatch::operation()` → `FileOperation::{Create, Delete, Modify{original,modified}, Rename{from,to}, Copy{from,to}}`; `old_mode()/new_mode()` → `FileMode`.
- `FilePatch::patch()` → `PatchKind::{Text(Patch), Binary(BinaryPatch)}` / `is_binary()`.
- For text: `Patch::hunks()` → `Hunk { old_range(), new_range(), function_context(), lines() }`; `Line::{Context, Delete, Insert}(&'a str)` — **borrowed, zero-copy**, feeds straight into the highlighter.
- **Why diffy over alternatives:** `unidiff` models metadata but no apply and a heavier model; `patch`/`gitpatch` *discard* extended headers (no rename/binary) and `gitpatch` has a single release. diffy also *applies* patches — free hedge toward a future "stage from rediff." Binary delta decoding is feature-gated and left off; only detection is used.

### Decision: New `src/pager.rs` adapter + ANSI renderer in `render.rs`
Pipeline: `stdin → diffy PatchSet → per file: reconstruct old/new line sequences from hunks → run the existing Highlight engine → emit ANSI`.
- The highlighter (`highlight::Highlight`, producing `FileHighlight`/`Span`/`Paint`) is reused as-is; pager mode is one-shot so no per-file cache (`tui/highlight.rs`) is needed.
- The ANSI emitter is a new function in `render.rs` next to `to_unified_string`: it walks hunk lines, asks the highlighter for each line's `[Span]`, and writes `Paint`→ANSI (SGR truecolor) with add/del/context backgrounds from the theme. Force-color is unconditional in this path (stdout is a pipe).

### Decision: CLI routing
Add `Command::Pager` (phase 1) and, later, `Command::Print` to `src/cli.rs`. In `src/main.rs`, route `pager`/`print` to the headless renderer **before** the `is_terminal()` TUI branch, so they never enter the TUI regardless of TTY. `--theme` is accepted for parity with the other subcommands; theme otherwise resolves from config.

## Risks / Trade-offs

- **Hunk-fragment highlighting** → a pager sees only a few context lines per hunk, so tree-sitter has less context than the TUI (which has whole files); tokens spanning above the hunk (open strings/comments/blocks) may mis-highlight at edges. Mitigation: accept it for v1 (delta lives with the same limitation); optionally later, parse the file path from the diff header and read the working-tree file for fuller context. `print` mode (phase 2) avoids this entirely (it gets whole files).
- **Deepest line-staging sub-view stays plain** → lazygit renders its line-by-line *staging cursor* view itself, bypassing the pager; rediff colors reach the main diff panel but not that sub-view. Mitigation: none needed — expected, identical to delta; staging still works.
- **diffy `patch_set` is a newer layer** (active development, FIXMEs about error-span precision) → low risk for our read-only use; the parsed model we rely on (operations, hunks, lines, binary flag) is stable and covered by its tests. Mitigation: pin `0.5`, add fixture tests over representative `git diff` outputs.
- **Color/encoding edge cases** (no-newline-at-eof, CRLF, non-UTF8) → mitigate by mirroring `to_unified_string`'s existing handling and adding fixtures; fall back to plain bytes for non-text we can't highlight.

## Migration Plan

1. Land `rediff pager` (additive subcommand; no change to existing behavior).
2. Update `dotfiles/dot_config/lazygit/config.yml`: replace the difft `externalDiffCommand` entry with `pager: rediff pager`. This is the moment line/hunk staging returns.
3. `just install` so the wired binary reflects the new subcommand.
4. Rollback: revert the lazygit config entry (re-add difft or go native); the subcommand is inert unless invoked.

## Open Questions

- Should v1 read the working-tree file for full-context highlighting, or ship fragment-only first? (Lean: fragment-only, revisit if edges bother in practice.)
- Phase-2 `rediff print`: wire via a git alias / `--ext-diff` on demand, keeping bare `git diff` plain — confirm the exact alias ergonomics when that phase is scoped.

## Outcome (corrections after implementation)

Both modes shipped; phase-2 `rediff print` shipped as **`rediff external`**. Hands-on testing corrected two assumptions above:

- **`externalDiffCommand` does not break per-file hunk staging.** lazygit renders its own diff for the line/hunk *staging view* (`--no-ext-diff`) in BOTH the pager and external paths, so staging works either way. The real differences are the *focused read-only* view (pager keeps rediff colors; external reverts to plain git on focus, #4724) and cost (pager = one invocation per view; external = one per file, rebuilding the highlight engine each time — ~7× slower on a 7-file diff).
- **Untracked files cannot appear in lazygit's combined view via any diff tool.** lazygit builds that view from `git diff`, which excludes untracked — measured identical for difft, `pager`, and `external`. So `external` does *not* restore untracked-in-combined. Untracked are visible per-file (select the file) and via the `<c-g>` rediff TUI; `git add -N` is the only way to surface them in the combined view.

**Final wiring:** lazygit uses `pager: rediff pager` (colors persist on focus, constant-time on large diffs); `rediff external` serves plain-terminal `git diff`/`show`/`log -p` and remains available as an alternative `externalDiffCommand`. One implementation note not anticipated above: a pager must strip ANSI from git's `--color=always` output before parsing (via `strip-ansi-escapes`), else nothing parses and the panel is blank.
