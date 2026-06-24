## Why

The two built-in themes (`dark`/`light`) are hand-maintained `Color::Rgb` literals, and the tree-sitter highlighter carries a second hand-tuned palette (`palette(name, dark)`) with only dark/light branches. Adding or refining themes means babysitting hex values in two places. We already depend on `two-face`, which bundles bat's full, upstream-maintained theme collection (~20 themes: Dracula, Nord, Gruvbox, Solarized, Monokai, …) as `syntect::Theme` values — but we only use it for syntaxes, ignoring its themes. Adopting that set removes the maintenance burden and lets one theme drive both highlighter paths and the UI chrome.

## What Changes

- Adopt `two_face::theme::extra()` as the theme registry (~20 maintained themes), replacing the two hand-coded `Theme` constructors.
- Unify both highlighter paths on a single `syntect::Theme`: keep syntect as-is, and bridge tree-sitter by mapping its capture names to TextMate scopes resolved against the same theme (a static 27-name → scope map written once, not per-theme).
- Derive UI **chrome** (foreground, selection, comment-muted, header/selection backgrounds) from the active theme's settings/scopes so chrome follows the theme.
- Diff add/del colors come from the theme's `markup.inserted`/`markup.deleted` scopes when **both** are present; otherwise fall back to a standard green/red. Diff backgrounds always blend toward the theme background so they stay harmonized either way.
- Keep the `local`/`commit` source accents (blue/green) standard across all themes — they encode *which kind of diff* you're viewing, a signal that must stay recognizable.
- **BREAKING**: the highlight cache key changes from `dark: bool` to theme identity (two dark themes no longer share cached spans), so theme switches re-resolve colors for visible content.
- Add a **live-preview theme picker** overlay: a multi-column grid navigated with arrows or `hjkl`; moving the cursor applies the theme to the whole UI immediately; `Enter` commits, `Esc` rolls back to the theme open at entry. `t` opens the picker (replacing the blind cycle).
- **Persist on commit**: when a theme is committed in the picker, write it to `~/.config/rediff/config.toml`, preserving existing keys and comments. Preview and cancel never write.

## Capabilities

### New Capabilities
- `theme-selection`: an interactive theme picker overlay with grid navigation, live whole-UI preview on cursor move, commit-to-apply, cancel-to-rollback, and persistence of the committed theme to the config file.

### Modified Capabilities
- `theming-and-config`: the built-in theme set becomes the adopted `two-face` collection; chrome follows the active theme with a standard diff add/del fallback; the config file becomes writable so a committed theme is persisted.
- `syntax-highlighting`: highlight colors are sourced from the active theme (both tree-sitter and syntect paths), and the per-file highlight cache is keyed by theme identity rather than a dark/light flag.

## Impact

- **Dependencies**: use `two-face`'s theme module (already a dependency); add `toml_edit` for comment-preserving config writes.
- **Code**: `src/highlight/engine.rs` (color sourcing, scope bridge, cache-relevant signature), `src/tui/theme.rs` (registry over enum-of-2, chrome derivation, cycle ring), `src/tui/highlight.rs` (cache key by theme), `src/tui/app.rs` + `src/tui/mod.rs` (new `Overlay::ThemePicker`, live preview, key routing), `src/tui/ui.rs` (picker rendering), `src/tui/keymap.rs` (`t` opens picker, `HINT_THEME`, `?`-help entry, consistency test), `src/config.rs` (write path).
- **Behavior**: `t` changes from instant cycle to opening the picker; theme switching now re-highlights visible content (or resolves at render time, per the design's live-preview decision).
