## 1. Theme registry (two-face adoption)

- [x] 1.1 Replace `ThemeName` enum-of-2 with an id over an ordered registry `themes::ALL` backed by `two_face::theme::extra()` (name lookup + index)
- [x] 1.2 Implement `parse()` by name with safe default; map legacy `"dark"`/`"light"` strings to chosen collection themes; unknown → default
- [x] 1.3 Implement `next()` to walk the ordered ring instead of flipping `dark`
- [x] 1.4 Expose the active `syntect::Theme` for the chosen `ThemeName` to the engine/chrome builders
- [x] 1.5 Unit tests: parse (legacy + unknown), ring cycle covers all entries and wraps

## 2. Highlighter unification (scope bridge)

- [x] 2.1 Add static `NAMES[i] → TextMate scope` map alongside the existing 27-entry `NAMES`
- [x] 2.2 At theme-load, resolve each scope against the active theme via `Highlighter::style_for_stack` into a `[Rgb; NAMES.len()]` table
- [x] 2.3 Switch the syntect path to use the active two-face theme (replace `ThemeSet::load_defaults()` selection)
- [x] 2.4 Change tree-sitter spans to carry the capture index; resolve `Rgb` at render time from the active theme's table (D5 / Option B)
- [x] 2.5 Fallback: if the capture-index span model proves too invasive, resolve color at highlight time and re-highlight on theme change (Option A) — keep cache-key-by-theme either way
- [x] 2.6 Tests: tree-sitter keyword/string/comment colors track the active theme; both paths follow a theme change

## 3. Highlight cache by theme identity

- [x] 3.1 Change the cache key in `tui/highlight.rs` from `dark: bool` to theme identity
- [x] 3.2 Ensure a theme change yields correct colors for visible content without blocking input
- [x] 3.3 Update `set_dark`/`reset` call sites to the theme-identity flow
- [x] 3.4 Tests: theme change invalidates/recolors as specified; same-theme re-display still served from cache

## 4. Chrome derivation + diff fallback

- [x] 4.1 Derive chrome fields from the active theme (foreground→context, comment→muted, keyword/function→accent/hunk, selection→sel_bg, lightened bg/selection→header_bg/sel_focus_bg, luminance(bg)→dark)
- [x] 4.2 Diff add/del: use `markup.inserted`/`markup.deleted` when BOTH present, else standard green/red by dark/light
- [x] 4.3 Compute `add_bg`/`del_bg`/`add_emph`/`del_emph` by blending add/del fg toward theme background in all cases
- [x] 4.4 Keep `local`/`commit` accents standard across themes
- [x] 4.5 Apply minimum-contrast delta when deriving selection/header backgrounds from low-contrast themes
- [x] 4.6 Tests: theme with diff scopes uses them; theme without falls back; backgrounds harmonized; accents unchanged

## 5. Theme picker overlay

- [x] 5.1 Add `Overlay::ThemePicker(ThemePicker)` with `{ items, selected, original }`; open on `t`, snapshot `original`, cursor on active theme
- [x] 5.2 Render the multi-column grid in `ui.rs`: columns from popup width, name + swatch (keyword/added/removed) per cell, highlight the cursor
- [x] 5.3 Route keys in `mod.rs::handle_key`: `h/←`,`l/→` = ∓1; `k/↑`,`j/↓` = ∓cols (clamped)
- [x] 5.4 Live preview: on cursor move, apply `items[selected]` to `app.theme` + highlighter so the whole UI re-renders
- [x] 5.5 `Enter` commits (close, keep, trigger persistence); `Esc` restores `original` and closes
- [x] 5.6 Tests: open snapshots original; navigation clamps; Esc rolls back; Enter keeps

## 6. Keymap, help, hints

- [x] 6.1 Change `t` from cycle to "open theme picker"; decide/keep or drop instant-cycle binding
- [x] 6.2 Add `HINT_THEME` status-line string and a `?`-help entry for the picker
- [x] 6.3 Update the keymap `consistency` test so help/hint/router stay in sync

## 7. Config persistence

- [x] 7.1 Add `toml_edit` dependency
- [x] 7.2 Add a config write path: load file as `DocumentMut`, set `doc["theme"]`, preserve other keys/comments, create dir/file if absent, write atomically (temp + rename)
- [x] 7.3 Wire the picker's commit to the write path; preview and cancel never write
- [x] 7.4 Surface a write failure as a status flash without crashing
- [x] 7.5 Tests: write preserves an existing `mode` key and comments; creates file when absent; failure path does not panic

## 8. Spec sync, validation, finalize

- [x] 8.1 Update existing specs/docs as needed and run `openspec validate adopt-theme-set-and-picker`
- [x] 8.2 `cargo test` green and `cargo clippy --all-targets` clean
- [x] 8.3 Run `just install` so the installed `rediff` reflects the change
