## Context

A "theme" in rediff bundles three things: UI **chrome** (the 19 `Color::Rgb` fields in `src/tui/theme.rs`), a **syntax palette**, and a **dark/light bit**. Today only chrome is per-theme; syntax colors live in two separate places:

- **tree-sitter path** (`engine.rs::palette(name, dark)`) — the primary path for bundled languages (rust/ts/tsx/js). 27 capture names → hand-tuned RGB, two branches (dark/light).
- **syntect path** (`engine.rs::syntect_highlight`) — the breadth fallback. Already consumes a `syntect::Theme` (`base16-ocean.dark` / `InspiredGitHub` from `ThemeSet::load_defaults()`).

`ThemeName` is an enum of two variants; `Theme::next()` is a binary flip; the highlight cache (`tui/highlight.rs`) keys on `dark: bool`. The TUI already has a clean overlay system: `Mode { base, overlay: Option<Overlay> }` with `Overlay::{Palette, Help}`, rendered in `ui.rs`, routed in `mod.rs::handle_key`. Config (`src/config.rs`) is read-only (`Deserialize` only), loaded from `~/.config/rediff/config.toml` (XDG).

`two-face` (already a dependency) exposes `two_face::theme::extra()` — bat's upstream-maintained collection as `syntect::Theme` values (Dracula, Nord, Gruvbox dark/light, Solarized dark/light, Monokai ×4, OneHalf dark/light, TwoDark, Coldark, Sublime Snazzy, Zenburn, GitHub, …, ~20 total).

## Goals / Non-Goals

**Goals:**
- Stop hand-maintaining theme colors: adopt `two-face`'s set as the source of truth.
- One active `syntect::Theme` drives both highlighter paths and the UI chrome.
- A live-preview theme picker: grid, arrows/`hjkl`, `Enter` commit, `Esc` rollback, whole-UI preview on cursor move.
- Persist the committed theme to the config file without destroying user comments/keys.

**Non-Goals:**
- User-authored or external theme files (only the bundled `two-face` set).
- Per-capability user color overrides.
- Changing the async highlight worker model, the diff engine, or the overlay framework itself.
- Theming the `local`/`commit` source accents (they stay standard by decision).

## Decisions

### D1: Theme registry from `two_face::theme::extra()`
Replace `ThemeName` enum-of-2 with a registry over the embedded set. `ThemeName` becomes a thin id (the theme's name string or an index into an ordered list `themes::ALL`). `parse()` looks up by name (default to a chosen dark theme on miss); `next()` walks the ordered ring instead of flipping a bool. Each entry resolves to a `two_face` `syntect::Theme`.

- *Why:* the set is maintained upstream; "add a theme" becomes a registry entry, never a hex value.
- *Alternative (rejected):* keep hand-coded `Theme::dark()/light()` and add more by hand — exactly the maintenance burden we're removing.

### D2: Bridge tree-sitter to the active `syntect::Theme` via a scope map
Keep the existing 27-entry `NAMES` list. Add a static `NAMES[i] → TextMate scope string` map (e.g. `keyword`→`keyword`, `function`→`entity.name.function`, `type`→`entity.name.type`, `string`→`string`, `comment`→`comment`, `number`→`constant.numeric`, `variable.parameter`→`variable.parameter`). At theme-load, resolve each scope against the theme with `syntect::highlighting::Highlighter::new(&theme).style_for_stack(...)` into a precomputed `[Rgb; NAMES.len()]` table. The hot path stays an array index.

- *Why:* one theme now colors both engines; the only ongoing artifact is the scope map (stable TextMate vocabulary, written once).
- *Alternative (rejected):* keep tree-sitter's own palette and only theme syntect — leaves the primary path hand-maintained and visually inconsistent with the fallback.

### D3: Chrome derived from the theme, with a standard diff fallback
Derive chrome from the theme: `context`←`settings.foreground`, `muted`←`comment` scope (else dimmed foreground), `accent`/`hunk`←`keyword`/`function` scope, `sel_bg`←`settings.selection`, `header_bg`/`sel_focus_bg`←background/selection lightened or brightened, `dark`←luminance(`settings.background`).

Diff add/del: **if the theme defines both `markup.inserted` and `markup.deleted`**, use those foregrounds; **otherwise** use a standard green/red (dark or light variant chosen by the `dark` bit). In **both** cases, `add_bg`/`del_bg`/`add_emph`/`del_emph` are computed by blending the add/del foreground toward the theme background, so diff rows stay harmonized with whatever canvas the theme paints.

- *Why:* chrome should follow the theme (user decision), but many bat themes omit diff scopes; a standard fallback keeps diffs readable everywhere, and the blend keeps them from clashing with the theme background.
- *Alternative (rejected):* require diff scopes — would break ~half the themes; or keep all chrome standard — chrome wouldn't follow the theme.

### D4: Keep `local`/`commit` accents standard
The blue=local/staged, green=commit accents (status bar/sidebar) signal *which kind of diff* is shown. Keep them as fixed standard colors across all themes rather than deriving per theme.

- *Why:* signal recognizability across 20 themes outweighs per-theme harmony (user decision).

### D5: Live preview by deferred color resolution (tree-sitter), re-highlight (syntect)
Theme switching must not feel laggy while sweeping the picker. The cache key changes from `dark: bool` to **theme identity** regardless, because two dark themes now differ in color.

Chosen approach (**Option B** from exploration): tree-sitter highlight spans store the **capture index** (0..27); the concrete `Rgb` is resolved at **render time** from the active theme's `[Rgb; 27]` table. A theme switch then swaps the table — **zero re-highlight** for bundled languages, so live preview is instant. The syntect fallback path produces `Rgb` directly (syntect resolves internally), so it re-highlights on theme change as before (acceptable: fallback languages, async, visible-only).

- *Why:* the picker's live preview is the feature that justifies decoupling color from parse; bundled languages are the common case and get instant theming.
- *Alternative (Option A, documented):* spans keep resolved `Rgb`; every theme switch invalidates the cache and re-highlights visible files. Simpler, smaller diff, but a perceptible re-highlight flicker while sweeping the grid. Fallback path if B proves too invasive to the span/render model.

### D6: Theme picker as a third `Overlay` variant
Add `Overlay::ThemePicker(ThemePicker)` to the existing enum; the `Option<Overlay>` "at most one overlay" invariant is preserved. State: `{ items: Vec<ThemeName>, selected: usize, original: ThemeName }`; grid columns computed from popup width at draw time. Routing in `mod.rs::handle_key`: `h/←`,`l/→` = ∓1; `k/↑`,`j/↓` = ∓cols (clamped); `Enter` commit; `Esc` restore `original`. Each cell shows the theme name plus a small swatch (keyword/added/removed). `t` opens the picker.

- *Why:* reuses the proven overlay framework; no new input-routing machinery.

### D7: Persist on commit with `toml_edit`
On `Enter` only, write the committed theme to `~/.config/rediff/config.toml`. Load the file as a `toml_edit::DocumentMut`, set `doc["theme"]`, write back; create the directory/file if absent; write atomically (temp + rename). Preview (cursor move) and cancel (`Esc`) never write. A write failure is surfaced as a status flash, not a crash — the in-session theme already applied.

- *Why:* the config is hand-editable; full reserialization would eat comments and any non-struct keys. `toml_edit` edits surgically.
- *Alternative (rejected):* derive `Serialize` on `Config` and rewrite the whole file — destroys comments and key ordering on the first theme pick.

### D8: Precedence unchanged
The picker writes the *persisted default*. CLI `--theme` still overrides at launch. After committing theme Y, the current session shows Y and the next launch without a flag starts in Y.

## Risks / Trade-offs

- **Span model change for D5** → If storing a capture index in tree-sitter spans proves too invasive to `Span`/render, fall back to Option A (re-highlight on switch); the cache-key-by-theme change stands either way, so live preview still works, just with visible-file re-highlight.
- **Themes without diff scopes** → standard add/del fallback + background blend keeps diffs readable; no theme is left with an unusable diff view.
- **Chrome from low-contrast themes** (e.g. a theme whose `selection` ≈ `background`) → derive `sel_focus_bg`/`header_bg` by lightening/brightening with a minimum delta so selection stays visible.
- **`t` semantics change** (cycle → picker) → documented in keymap help and `HINT_THEME`; the `consistency` test enforces help/hint/router stay in sync, so the change can't silently drift.
- **Config write to a read-only FS** → status flash, session theme still applies; never crash.
- **New dependency `toml_edit`** → small, widely used, shares lineage with the existing `toml` crate; scoped to `config.rs` writes.

## Migration Plan

1. Land the registry + bridge + chrome derivation behind the existing `Theme`/`ThemeName` API surface so the rest of the TUI keeps reading `app.theme.<field>`.
2. Switch the highlight cache key to theme identity; implement D5 (deferred resolution for tree-sitter).
3. Add the picker overlay, key routing, rendering, keymap/help/hint updates.
4. Add the config write path.
5. No data migration: existing `theme = "dark"|"light"` config values still parse (map to a default dark/light theme in the new registry); unknown names default safely.

## Open Questions

- Final swatch content per grid cell (keyword/added/removed vs. a longer strip) — cosmetic, settle during picker rendering.
- Exact default theme chosen for the `dark`/`light` legacy config strings (e.g. map `"dark"`→`TwoDark` or a GitHub-dark) — pick during implementation to best match the current look.
