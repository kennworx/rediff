//! The theme registry and chrome palette. Themes are adopted wholesale from the
//! bundled `two-face` (bat) collection — we resolve each to a `syntect::Theme`
//! and *derive* the UI chrome from it, rather than hand-maintaining colors. Diff
//! add/del colors fall back to a standard green/red when a theme does not define
//! them, and the source accents stay standard across all themes.

use std::sync::OnceLock;

use ratatui::style::Color;
use two_face::theme::EmbeddedThemeName as Tf;

use crate::highlight::{self, Rgb};

/// The curated, ordered theme set. Index 0 is the default. ANSI/base16-256/1337
/// are excluded — they are terminal-palette dependent and do not render as
/// truecolor chrome. `ThemeName` is an index into this list.
const ALL: &[Tf] = &[
    Tf::TwoDark,
    Tf::Github,
    Tf::Dracula,
    Tf::Nord,
    Tf::GruvboxDark,
    Tf::GruvboxLight,
    Tf::SolarizedDark,
    Tf::SolarizedLight,
    Tf::MonokaiExtended,
    Tf::OneHalfDark,
    Tf::OneHalfLight,
    Tf::CatppuccinMocha,
    Tf::CatppuccinMacchiato,
    Tf::CatppuccinFrappe,
    Tf::CatppuccinLatte,
    Tf::ColdarkDark,
    Tf::ColdarkCold,
    Tf::SublimeSnazzy,
    Tf::Zenburn,
    Tf::Base16OceanDark,
    Tf::Base16EightiesDark,
    Tf::Base16MochaDark,
    Tf::Base16OceanLight,
    Tf::InspiredGithub,
    Tf::MonokaiExtendedBright,
    Tf::MonokaiExtendedLight,
    Tf::MonokaiExtendedOrigin,
    Tf::DarkNeon,
];

/// Which built-in theme to use: an index into `ALL`. Parsed from the CLI/config
/// string at the boundary; everything inside the TUI passes this id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThemeName(usize);

#[expect(
    non_upper_case_globals,
    reason = "associated consts mirror enum-variant naming for ergonomic ThemeName::Dark / ::Light"
)]
impl ThemeName {
    /// The default dark theme (legacy `"dark"`).
    pub const Dark: ThemeName = ThemeName(0);
    /// The default light theme (legacy `"light"`).
    pub const Light: ThemeName = ThemeName(1);

    /// Parse a CLI/config theme string: legacy `dark`/`light`, an exact theme
    /// name (case-insensitive), else the default.
    pub fn parse(s: &str) -> ThemeName {
        let s = s.trim();
        match s.to_lowercase().as_str() {
            "dark" => ThemeName::Dark,
            "light" => ThemeName::Light,
            lower => ALL
                .iter()
                .position(|t| t.as_name().to_lowercase() == lower)
                .map(ThemeName)
                .unwrap_or_default(),
        }
    }

    /// The underlying `two-face` theme.
    #[expect(
        clippy::indexing_slicing,
        reason = "self.0 is always a valid index into ALL by construction"
    )]
    fn embedded(self) -> Tf {
        ALL[self.0]
    }

    /// The human-readable theme name (for the picker and persistence).
    pub fn display(self) -> &'static str {
        self.embedded().as_name()
    }

    /// The per-capture syntax color table for this theme (indexed by
    /// `highlight::Paint::Capture`).
    pub fn syntax_table(self) -> Vec<Rgb> {
        highlight::syntax_table(highlight::themes().get(self.embedded()))
    }

    /// The `two-face` id, for handing the active theme to the highlight worker.
    pub fn embedded_name(self) -> Tf {
        self.embedded()
    }

    /// Whether this theme has a dark canvas (background luminance below mid).
    /// Cheap — reads the theme background without building the full chrome.
    pub fn is_dark(self) -> bool {
        let st = highlight::themes().get(self.embedded());
        luminance(opt(st.settings.background, (13, 17, 23))) < 0.5
    }

    /// This theme's position within its own brightness tab (for placing the
    /// picker cursor when the picker opens on the active theme).
    pub fn position_in_tab(self) -> usize {
        themes_by_brightness(self.is_dark())
            .iter()
            .position(|&n| n == self)
            .unwrap_or(0)
    }
}

/// The themes of a given brightness, in registry order — the two picker tabs.
/// Classified once (luminance of each theme's background) and cached.
pub fn themes_by_brightness(dark: bool) -> &'static [ThemeName] {
    static PARTITION: OnceLock<(Vec<ThemeName>, Vec<ThemeName>)> = OnceLock::new();
    let (d, l) = PARTITION.get_or_init(|| {
        let mut d = Vec::new();
        let mut l = Vec::new();
        for i in 0..ALL.len() {
            let n = ThemeName(i);
            if n.is_dark() {
                d.push(n);
            } else {
                l.push(n);
            }
        }
        (d, l)
    });
    if dark {
        d
    } else {
        l
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    /// The theme this palette was built from (for cycling and the picker).
    pub name: ThemeName,
    pub dark: bool,
    /// The theme's canvas background (for popups that must own their background
    /// so text contrast holds regardless of the terminal's default background).
    pub bg: Color,
    pub added: Color,
    pub removed: Color,
    pub hunk: Color,
    pub muted: Color,
    pub accent: Color,
    pub context: Color,
    pub warn: Color,
    pub purple: Color,
    pub add_bg: Color,
    pub del_bg: Color,
    /// Stronger backgrounds for word-level intra-line emphasis.
    pub add_emph: Color,
    pub del_emph: Color,
    pub header_bg: Color,
    pub sel_bg: Color,
    pub sel_focus_bg: Color,
    /// Source accent for local/staged views (blue).
    pub local: Color,
    /// Source accent for commit/range views (green).
    pub commit: Color,
}

impl Theme {
    /// Build the chrome palette for a theme, derived from its `syntect::Theme`.
    pub fn new(name: ThemeName) -> Theme {
        let st = highlight::themes().get(name.embedded());
        let s = &st.settings;

        let bg = opt(s.background, (13, 17, 23));
        let fg = highlight::theme_fg(st);
        let dark = luminance(bg) < 0.5;

        // Diff add/del: theme scopes when BOTH are defined, else standard.
        let (added, removed) = if highlight::scope_defined(st, "markup.inserted")
            && highlight::scope_defined(st, "markup.deleted")
        {
            (
                highlight::scope_color(st, "markup.inserted"),
                highlight::scope_color(st, "markup.deleted"),
            )
        } else if dark {
            ((63, 185, 80), (248, 81, 73))
        } else {
            ((26, 127, 55), (207, 34, 46))
        };

        // Backgrounds always blend toward the theme background so diff rows stay
        // harmonized with whatever canvas the theme paints.
        let add_bg = blend(added, bg, 0.84);
        let del_bg = blend(removed, bg, 0.84);
        let add_emph = blend(added, bg, 0.62);
        let del_emph = blend(removed, bg, 0.62);

        let muted = highlight::scope_color(st, "comment");
        let accent = highlight::scope_color(st, "entity.name.function");
        let sel_bg = opt(s.selection, blend(bg, fg, 0.12));
        let header_bg = blend(bg, fg, 0.06);
        let sel_focus_bg = blend(sel_bg, fg, 0.18);

        // Standard, theme-independent: source accents (kind-of-diff signal) and
        // the status-glyph warn/purple.
        let (local, commit, warn, purple) = if dark {
            (
                (110, 168, 254),
                (219, 138, 224),
                (210, 153, 34),
                (210, 168, 255),
            )
        } else {
            ((9, 105, 218), (154, 56, 173), (154, 103, 0), (130, 80, 223))
        };

        Theme {
            name,
            dark,
            bg: rgb(bg),
            added: rgb(added),
            removed: rgb(removed),
            hunk: rgb(accent),
            muted: rgb(muted),
            accent: rgb(accent),
            context: rgb(fg),
            warn: rgb(warn),
            purple: rgb(purple),
            add_bg: rgb(add_bg),
            del_bg: rgb(del_bg),
            add_emph: rgb(add_emph),
            del_emph: rgb(del_emph),
            header_bg: rgb(header_bg),
            sel_bg: rgb(sel_bg),
            sel_focus_bg: rgb(sel_focus_bg),
            local: rgb(local),
            commit: rgb(commit),
        }
    }

    /// The default foreground as an `Rgb`, for resolving `highlight::Paint::Default`.
    pub fn context_rgb(&self) -> Rgb {
        match self.context {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (200, 200, 200),
        }
    }
}

fn rgb(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// A syntect optional color, or a fallback.
fn opt(c: Option<syntect::highlighting::Color>, fallback: Rgb) -> Rgb {
    c.map_or(fallback, |c| (c.r, c.g, c.b))
}

/// Relative luminance (0.0–1.0), for dark/light classification.
fn luminance(c: Rgb) -> f32 {
    let f = |v: u8| f32::from(v) / 255.0;
    0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
}

/// Blend `a` toward `b` by `t` (0.0 = all `a`, 1.0 = all `b`).
fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "blend of two u8 channels with t in [0,1] stays in 0..=255"
    )]
    let m = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_and_named_and_unknown() {
        assert_eq!(ThemeName::parse("dark"), ThemeName::Dark);
        assert_eq!(ThemeName::parse("light"), ThemeName::Light);
        assert_eq!(ThemeName::parse("Dracula").display(), "Dracula");
        assert_eq!(ThemeName::parse("Nord").display(), "Nord");
        // Unknown falls back to the default without error.
        assert_eq!(ThemeName::parse("nonsense"), ThemeName::default());
    }

    #[test]
    fn dark_and_light_classify_correctly() {
        assert!(Theme::new(ThemeName::Dark).dark);
        assert!(!Theme::new(ThemeName::Light).dark);
    }

    #[test]
    fn every_theme_builds_and_is_partitioned() {
        // The two brightness tabs together cover the whole registry, and every
        // theme builds its chrome without panicking.
        let dark = themes_by_brightness(true);
        let light = themes_by_brightness(false);
        assert_eq!(
            dark.len() + light.len(),
            ALL.len(),
            "partition covers every theme"
        );
        for &name in dark.iter().chain(light) {
            let _ = Theme::new(name);
        }
    }

    #[test]
    fn context_rgb_reads_rgb_and_falls_back() {
        let mut t = Theme::new(ThemeName::Dark);
        // The derived chrome's context is always an Rgb color → its channels.
        match t.context {
            Color::Rgb(r, g, b) => assert_eq!(t.context_rgb(), (r, g, b)),
            other => panic!("derived context should be Rgb, got {other:?}"),
        }
        // A non-Rgb context falls back to a neutral gray.
        t.context = Color::Reset;
        assert_eq!(t.context_rgb(), (200, 200, 200));
    }
}
