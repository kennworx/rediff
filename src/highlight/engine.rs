//! The concrete highlighting engine: tree-sitter for bundled languages,
//! syntect (two-face syntaxes) as the breadth fallback, plain text otherwise.
//!
//! Colors come from the active theme (a `two-face`/syntect theme), not a
//! hand-coded palette. The tree-sitter path emits theme-independent capture
//! indices (`Paint::Capture`) resolved at render time via [`syntax_table`]; the
//! syntect path bakes concrete colors (`Paint::Fixed`).

use std::collections::HashMap;
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Highlighter, Theme as SynTheme};
use syntect::parsing::{Scope, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter};
use two_face::theme::{EmbeddedLazyThemeSet, EmbeddedThemeName};

use super::{Highlight, Lines, Paint, Rgb, Span};

/// tree-sitter capture names we configure and color. `Paint::Capture(i)` indexes
/// into this list; [`syntax_table`] resolves each to a color, in the same order.
const NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "embedded",
    "function",
    "function.builtin",
    "function.method",
    "keyword",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "label",
    "module",
    "escape",
];

/// Map a tree-sitter capture key (the part before the first `.`) to a `TextMate`
/// scope, resolved against the active theme. `""` means "use the theme's default
/// foreground". This is the only ongoing per-capture artifact — stable `TextMate`
/// vocabulary, written once, not per-theme.
fn capture_scope(key: &str) -> &'static str {
    match key {
        "keyword" => "keyword",
        "string" => "string",
        "escape" => "constant.character.escape",
        "comment" => "comment",
        "function" | "constructor" => "entity.name.function",
        "type" => "entity.name.type",
        "number" => "constant.numeric",
        "constant" => "constant",
        "property" | "attribute" | "label" => "variable.other.member",
        "tag" => "entity.name.tag",
        "module" => "entity.name.namespace",
        "variable" => "variable",
        _ => "",
    }
}

/// The bundled theme collection (`two-face`/bat), built once.
pub fn themes() -> &'static EmbeddedLazyThemeSet {
    static THEMES: OnceLock<EmbeddedLazyThemeSet> = OnceLock::new();
    THEMES.get_or_init(two_face::theme::extra)
}

/// The active theme's default foreground (empty scope stack).
pub fn theme_fg(theme: &SynTheme) -> Rgb {
    let c = Highlighter::new(theme).style_for_stack(&[]).foreground;
    (c.r, c.g, c.b)
}

/// Resolve a single `TextMate` scope to a foreground color in `theme`.
pub fn scope_color(theme: &SynTheme, scope: &str) -> Rgb {
    let hl = Highlighter::new(theme);
    let c = match Scope::new(scope) {
        Ok(s) => hl.style_for_stack(&[s]).foreground,
        Err(_) => hl.style_for_stack(&[]).foreground,
    };
    (c.r, c.g, c.b)
}

/// Whether `theme` explicitly themes `scope` (its color differs from the default
/// foreground). Used to decide whether diff add/del colors come from the theme.
pub fn scope_defined(theme: &SynTheme, scope: &str) -> bool {
    let hl = Highlighter::new(theme);
    let def = hl.style_for_stack(&[]).foreground;
    match Scope::new(scope) {
        Ok(s) => {
            let c = hl.style_for_stack(&[s]).foreground;
            (c.r, c.g, c.b) != (def.r, def.g, def.b)
        }
        Err(_) => false,
    }
}

/// The per-capture color table for `theme`, indexed by `Paint::Capture(i)` (and
/// thus by position in `NAMES`). Rebuilt cheaply when the active theme changes.
pub fn syntax_table(theme: &SynTheme) -> Vec<Rgb> {
    let hl = Highlighter::new(theme);
    let def = hl.style_for_stack(&[]).foreground;
    let def = (def.r, def.g, def.b);
    NAMES
        .iter()
        .map(|name| {
            let key = name.split('.').next().unwrap_or(name);
            match capture_scope(key) {
                "" => def,
                scope => match Scope::new(scope) {
                    Ok(s) => {
                        let c = hl.style_for_stack(&[s]).foreground;
                        (c.r, c.g, c.b)
                    }
                    Err(_) => def,
                },
            }
        })
        .collect()
}

pub struct Engine {
    ts: HashMap<&'static str, HighlightConfiguration>,
    syntaxes: SyntaxSet,
}

impl Engine {
    /// Build the engine. Called once on the highlight worker thread.
    pub fn new() -> Self {
        let mut ts = HashMap::new();

        let mut add = |key: &'static str, mut cfg: HighlightConfiguration| {
            cfg.configure(NAMES);
            ts.insert(key, cfg);
        };

        if let Ok(cfg) = HighlightConfiguration::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "",
        ) {
            add("rust", cfg);
        }

        let tsx_query = format!(
            "{}\n{}\n{}",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        );
        if let Ok(cfg) = HighlightConfiguration::new(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            &tsx_query,
            "",
            "",
        ) {
            add("tsx", cfg);
        }
        let typescript_query = format!(
            "{}\n{}",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
        );
        if let Ok(cfg) = HighlightConfiguration::new(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            &typescript_query,
            "",
            "",
        ) {
            add("typescript", cfg);
        }
        let js_query = format!(
            "{}\n{}",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        );
        if let Ok(cfg) = HighlightConfiguration::new(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            &js_query,
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ) {
            add("javascript", cfg);
        }

        let syntaxes = two_face::syntax::extra_newlines();
        Engine { ts, syntaxes }
    }

    /// Whether a language is highlighted via the syntect path, whose results bake
    /// theme colors and must be recomputed on a theme change. Tree-sitter and
    /// plain results are theme-independent.
    pub fn theme_dependent(&self, lang: Option<&str>) -> bool {
        match lang {
            Some(l) => Self::ts_key(l).is_none() && self.syntect_syntax(l).is_some(),
            None => false,
        }
    }

    /// Resolve a language id to a bundled tree-sitter config key.
    fn ts_key(lang: &str) -> Option<&'static str> {
        match lang {
            "rust" => Some("rust"),
            "tsx" => Some("tsx"),
            "typescript" => Some("typescript"),
            "javascript" | "jsx" => Some("javascript"),
            _ => None,
        }
    }

    fn syntect_syntax(&self, lang: &str) -> Option<&SyntaxReference> {
        self.syntaxes
            .find_syntax_by_token(lang)
            .or_else(|| self.syntaxes.find_syntax_by_extension(lang))
    }

    fn ts_highlight(cfg: &HighlightConfiguration, text: &str) -> Lines {
        let mut hl = TsHighlighter::new();
        let mut lines: Lines = Vec::new();
        let mut cur: Vec<Span> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();

        let Ok(events) = hl.highlight(cfg, text.as_bytes(), None, |_| None) else {
            return plain(text);
        };
        for ev in events {
            match ev {
                Ok(HighlightEvent::HighlightStart(h)) => stack.push(h.0),
                Ok(HighlightEvent::HighlightEnd) => {
                    stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "capture index is bounded by the small fixed NAMES table, far below u16::MAX"
                    )]
                    let paint = stack
                        .last()
                        .map_or(Paint::Default, |&i| Paint::Capture(i as u16));
                    #[expect(
                        clippy::string_slice,
                        reason = "tree-sitter Source byte offsets always fall on UTF-8 boundaries"
                    )]
                    let slice = &text[start..end];
                    let mut first = true;
                    for part in slice.split('\n') {
                        if !first {
                            lines.push(std::mem::take(&mut cur));
                        }
                        first = false;
                        if !part.is_empty() {
                            cur.push(Span {
                                text: part.to_string(),
                                paint,
                            });
                        }
                    }
                }
                Err(_) => return plain(text),
            }
        }
        lines.push(cur);
        lines
    }

    fn syntect_highlight(&self, syntax: &SyntaxReference, text: &str, theme: &SynTheme) -> Lines {
        let default_fg = theme_fg(theme);
        let mut h = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();
        for line in LinesWithEndings::from(text) {
            let spans = match h.highlight_line(line, &self.syntaxes) {
                Ok(ranges) => ranges
                    .into_iter()
                    .map(|(style, s)| Span {
                        text: s.trim_end_matches(['\n', '\r']).to_string(),
                        paint: Paint::Fixed((
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        )),
                    })
                    .filter(|s| !s.text.is_empty())
                    .collect(),
                Err(_) => vec![Span {
                    text: line.trim_end_matches(['\n', '\r']).to_string(),
                    paint: Paint::Fixed(default_fg),
                }],
            };
            lines.push(spans);
        }
        lines
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlight for Engine {
    fn highlight(&self, text: &str, lang: Option<&str>, theme: EmbeddedThemeName) -> Lines {
        if let Some(lang) = lang {
            if let Some(key) = Self::ts_key(lang) {
                if let Some(cfg) = self.ts.get(key) {
                    return Self::ts_highlight(cfg, text);
                }
            }
            if let Some(syntax) = self.syntect_syntax(lang) {
                return self.syntect_highlight(syntax, text, themes().get(theme));
            }
        }
        plain(text)
    }
}

/// One default-foreground span per line — the no-highlighter fallback. The color
/// is resolved at render time, so this is theme-independent.
fn plain(text: &str) -> Lines {
    text.split('\n')
        .map(|l| {
            let l = l.trim_end_matches('\r');
            if l.is_empty() {
                Vec::new()
            } else {
                vec![Span {
                    text: l.to_string(),
                    paint: Paint::Default,
                }]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_keywords() {
        let engine = Engine::new();
        let lines = engine.highlight("fn main() {}\n", Some("rust"), EmbeddedThemeName::TwoDark);
        assert!(!lines.is_empty());
        // the first line should contain multiple tokenized spans (fn, main, etc.)
        assert!(
            lines[0].len() > 1,
            "rust line should be tokenized into spans"
        );
        // `fn` is a keyword capture (theme-independent index, resolved at render)
        assert!(lines[0]
            .iter()
            .any(|s| s.text == "fn" && matches!(s.paint, Paint::Capture(_))));
    }

    #[test]
    fn unknown_language_is_plain() {
        let engine = Engine::new();
        let lines = engine.highlight(
            "hello world\n",
            Some("nonsense-lang"),
            EmbeddedThemeName::TwoDark,
        );
        assert_eq!(lines[0].len(), 1);
        assert_eq!(lines[0][0].paint, Paint::Default);
    }

    #[test]
    fn syntax_table_has_a_slot_per_capture() {
        let theme = themes().get(EmbeddedThemeName::TwoDark);
        assert_eq!(syntax_table(theme).len(), NAMES.len());
    }

    #[test]
    fn default_builds_a_working_engine() {
        // Exercise `Engine::default` (delegates to `new`) and confirm the
        // tree-sitter config map is populated for a bundled language.
        let engine = Engine::default();
        let lines = engine.highlight("let x = 1;\n", Some("rust"), EmbeddedThemeName::TwoDark);
        assert!(!lines.is_empty());
    }

    #[test]
    fn highlight_covers_ts_syntect_and_plain_paths() {
        let engine = Engine::new();

        // tree-sitter path: a config exists for "rust", so we get capture indices.
        let ts = engine.highlight(
            "fn main() { let s = \"hi\"; }\n",
            Some("rust"),
            EmbeddedThemeName::TwoDark,
        );
        assert!(ts[0].iter().any(|s| matches!(s.paint, Paint::Capture(_))));

        // syntect fallback path: "python" is not a tree-sitter key but is a
        // known syntect token, so spans carry baked `Fixed` colors.
        let syn = engine.highlight(
            "def foo():\n    return 1\n",
            Some("python"),
            EmbeddedThemeName::TwoDark,
        );
        assert!(syn
            .iter()
            .flatten()
            .any(|s| matches!(s.paint, Paint::Fixed(_))));

        // plain path: a recognized-by-nobody language id falls through to plain.
        let plainish = engine.highlight(
            "just text\n",
            Some("totally-unknown-xyz"),
            EmbeddedThemeName::TwoDark,
        );
        assert_eq!(plainish[0][0].paint, Paint::Default);

        // plain path: no language at all.
        let none = engine.highlight("just text\n", None, EmbeddedThemeName::TwoDark);
        assert_eq!(none[0][0].paint, Paint::Default);
    }

    #[test]
    fn syntect_highlight_directly_bakes_colors() {
        // Drive `Engine::syntect_highlight` through a language that routes to
        // syntect (not tree-sitter). Use a multi-line snippet so the per-line
        // loop runs more than once and we get several styled runs.
        let engine = Engine::new();
        let theme = themes().get(EmbeddedThemeName::TwoDark);
        let syntax = engine
            .syntect_syntax("python")
            .expect("python syntax should be bundled by two-face");
        let lines = engine.syntect_highlight(
            syntax,
            "import os\n\ndef greet(name):\n    return name\n",
            theme,
        );
        assert!(lines.len() >= 4, "one entry per source line");
        assert!(lines
            .iter()
            .flatten()
            .all(|s| matches!(s.paint, Paint::Fixed(_))));
        assert!(lines.iter().flatten().any(|s| !s.text.is_empty()));
    }

    #[test]
    fn theme_dependent_distinguishes_paths() {
        let engine = Engine::new();
        // None language is never theme-dependent.
        assert!(!engine.theme_dependent(None));
        // tree-sitter language: theme-independent (capture indices).
        assert!(!engine.theme_dependent(Some("rust")));
        // unknown language: not handled by either, so not theme-dependent.
        assert!(!engine.theme_dependent(Some("totally-unknown-xyz")));
        // syntect-only language bakes colors, so it IS theme-dependent.
        assert!(engine.theme_dependent(Some("python")));
    }

    #[test]
    fn scope_color_handles_valid_and_invalid_scopes() {
        let theme = themes().get(EmbeddedThemeName::TwoDark);
        // Valid scope resolves through the `Ok` arm.
        let _kw = scope_color(theme, "keyword");
        // Invalid scope (>8 atoms) takes the `Err` arm and falls back to default fg.
        let invalid = "a.b.c.d.e.f.g.h.i.j";
        assert_eq!(scope_color(theme, invalid), theme_fg(theme));
    }

    #[test]
    fn syntect_highlight_falls_back_on_a_parse_error() {
        // A syntax whose rule pushes a context that does not exist makes syntect's
        // parser return an Err for every line. Building an Engine directly over a
        // SyntaxSet containing this syntax (the test module sees the private
        // fields) and passing that syntax drives `syntect_highlight`'s error arm,
        // which falls back to a single default-foreground span per line.
        use std::collections::HashMap;
        use syntect::parsing::SyntaxSetBuilder;

        let yaml = "name: Boom\nscope: source.boom\nfile_extensions: [boom]\n\
                    contexts:\n  main:\n    - match: 'a'\n      push: nonexistent\n";
        let syndef = syntect::parsing::SyntaxDefinition::load_from_str(yaml, true, None)
            .expect("load custom syntax");
        let mut builder = SyntaxSetBuilder::new();
        builder.add(syndef);
        let engine = Engine {
            ts: HashMap::new(),
            syntaxes: builder.build(),
        };
        let theme = themes().get(EmbeddedThemeName::TwoDark);
        let syntax = engine
            .syntaxes
            .syntaxes()
            .first()
            .expect("custom syntax present");

        let lines = engine.syntect_highlight(syntax, "aaa\nbbb\n", theme);
        assert_eq!(lines.len(), 2, "one fallback entry per source line");
        let fg = theme_fg(theme);
        assert!(
            lines.iter().flatten().all(|s| s.paint == Paint::Fixed(fg)),
            "every span uses the default-foreground fallback"
        );
        assert!(
            lines.iter().flatten().any(|s| !s.text.is_empty()),
            "fallback keeps the line text"
        );
    }

    #[test]
    fn scope_defined_covers_all_arms() {
        let theme = themes().get(EmbeddedThemeName::TwoDark);
        // A themed scope differs from default fg -> true (Ok + differs).
        assert!(scope_defined(theme, "keyword"));
        // A valid-but-unthemed scope resolves to default fg -> false (Ok + equal).
        assert!(!scope_defined(theme, "this.scope.is.not.themed"));
        // An invalid scope (>8 atoms) -> false (Err arm).
        assert!(!scope_defined(theme, "a.b.c.d.e.f.g.h.i.j"));
    }
}
