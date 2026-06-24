//! Map a file path to a language id used later by the highlighter.

/// Best-effort language id from a path's extension. Returns `None` when unknown.
pub fn detect(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?;
    let lang = match ext {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" | "hh" => "cpp",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "sh" | "bash" => "bash",
        "html" => "html",
        "css" => "css",
        _ => return None,
    };
    Some(lang.to_string())
}
