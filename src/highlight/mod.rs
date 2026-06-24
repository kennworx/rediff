//! Syntax highlighting: a pluggable engine that produces per-line styled spans.
//! tree-sitter is primary for bundled languages; syntect is the breadth fallback.
//! All highlighting is one-shot per file (read-only viewer) and meant to run on
//! a worker thread — see `crate::tui::highlight`.

mod engine;

pub use engine::{scope_color, scope_defined, syntax_table, theme_fg, themes, Engine};

/// An RGB color for a styled run.
pub type Rgb = (u8, u8, u8);

/// How a styled run is colored. Tree-sitter spans carry a theme-independent
/// capture index (resolved at render time against the active theme's table), so
/// switching themes recolors cached content with no re-highlight. The syntect
/// path bakes a concrete color (`Fixed`), and everything else falls back to the
/// theme's default foreground (`Default`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// Index into the active theme's capture-color table (see `engine::syntax_table`).
    Capture(u16),
    /// The active theme's default foreground.
    Default,
    /// An already-resolved color (syntect-highlighted run).
    Fixed(Rgb),
}

/// A styled run of text within a single line.
#[derive(Debug, Clone)]
pub struct Span {
    pub text: String,
    pub paint: Paint,
}

/// Per-line styled spans for one file (index 0 == file line 1).
pub type Lines = Vec<Vec<Span>>;

/// Highlighting for both sides of a file's diff.
#[derive(Debug, Clone, Default)]
pub struct FileHighlight {
    pub old: Lines,
    pub new: Lines,
    /// Whether these spans carry theme-baked colors (`Paint::Fixed`, the syntect
    /// path) and must be recomputed when the active theme changes. Tree-sitter
    /// and plain results are theme-independent and survive a theme switch.
    pub theme_dependent: bool,
}

impl FileHighlight {
    /// Spans for a 1-based line on the given side, if available.
    pub fn line(&self, side_new: bool, lineno: u32) -> Option<&[Span]> {
        let lines = if side_new { &self.new } else { &self.old };
        lineno
            .checked_sub(1)
            .and_then(|i| lines.get(i as usize))
            .map(std::vec::Vec::as_slice)
    }
}

/// A highlighter turns file text + language id into per-line spans. The theme
/// only affects the syntect fallback path (which bakes concrete colors);
/// tree-sitter and plain results are theme-independent.
pub trait Highlight {
    fn highlight(
        &self,
        text: &str,
        lang: Option<&str>,
        theme: two_face::theme::EmbeddedThemeName,
    ) -> Lines;
}
