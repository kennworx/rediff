//! Persisted preferences from `~/.config/rediff/config.toml`. Missing or
//! malformed config falls back to defaults without error. CLI flags override.

use std::io;
use std::path::PathBuf;

use serde::Deserialize;

use crate::model::LayoutMode;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// "dark" | "light".
    pub theme: Option<String>,
    /// "split" | "stack" (or "auto"/unset to pick by terminal width at startup).
    pub mode: Option<String>,
}

impl Config {
    /// Load config from the standard path, returning defaults if absent/invalid.
    pub fn load() -> Config {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("rediff").join("config.toml"))
    }

    pub fn layout_mode(&self) -> Option<LayoutMode> {
        self.mode.as_deref().and_then(parse_mode)
    }

    /// Persist the `theme` preference to the config file, preserving existing
    /// keys and comments. Creates the directory/file if absent and writes
    /// atomically (temp + rename) so a crash never truncates the config.
    pub fn save_theme(theme: &str) -> io::Result<()> {
        let path =
            Self::path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config dir"))?;
        write_theme(&path, theme)
    }
}

/// Surgically set `theme` in the TOML document at `path` (or a fresh one),
/// preserving everything else. A missing file starts a fresh document; an
/// existing-but-malformed file is an error, NOT silently overwritten — clobbering
/// a user's hand-edited config (with a typo) would lose their other keys/comments.
#[expect(
    clippy::indexing_slicing,
    reason = "toml_edit's IndexMut<&str> inserts the key if absent; it never panics"
)]
fn write_theme(path: &std::path::Path, theme: &str) -> io::Result<()> {
    let mut doc = match std::fs::read_to_string(path) {
        Ok(existing) => existing
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => toml_edit::DocumentMut::new(),
        Err(e) => return Err(e),
    };
    doc["theme"] = toml_edit::value(theme);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, path)
}

/// Parse an explicit layout-mode string. `"auto"` (and anything unrecognized)
/// returns `None`, meaning "pick by terminal width at startup".
pub fn parse_mode(s: &str) -> Option<LayoutMode> {
    match s.to_lowercase().as_str() {
        "split" => Some(LayoutMode::Split),
        "stack" => Some(LayoutMode::Stack),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toml_config() {
        let cfg: Config = toml::from_str("theme = \"light\"\nmode = \"split\"").unwrap();
        assert_eq!(cfg.theme.as_deref(), Some("light"));
        assert_eq!(cfg.layout_mode(), Some(LayoutMode::Split));
    }

    #[test]
    fn empty_and_invalid_default_safely() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.theme.is_none());
        assert_eq!(cfg.layout_mode(), None);
        assert_eq!(parse_mode("nonsense"), None);
    }

    #[test]
    fn write_theme_preserves_other_keys_and_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# my prefs\nmode = \"split\"\ntheme = \"dark\"\n").unwrap();

        super::write_theme(&path, "Dracula").unwrap();

        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("# my prefs"), "comment preserved");
        assert!(out.contains("mode = \"split\""), "other key preserved");
        assert!(out.contains("theme = \"Dracula\""), "theme updated");
    }

    #[test]
    fn write_theme_creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        super::write_theme(&path, "Nord").unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("theme = \"Nord\""));
    }

    #[test]
    fn write_theme_propagates_a_read_error_other_than_missing() {
        // The "path" is a directory, so reading it as a string fails with an
        // error whose kind is NOT NotFound → the `Err(e) => return Err(e)` arm.
        let dir = tempfile::tempdir().unwrap();
        let err = super::write_theme(dir.path(), "Dracula").unwrap_err();
        assert_ne!(
            err.kind(),
            io::ErrorKind::NotFound,
            "a non-missing read error is propagated, not treated as a fresh file"
        );
    }

    #[test]
    fn path_prefers_xdg_then_home_then_none() {
        // These env vars are read only by Config::path, which no other test
        // exercises; save and restore them around the mutation.
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let saved_home = std::env::var_os("HOME");

        std::env::set_var("XDG_CONFIG_HOME", "/xdg/conf");
        assert_eq!(
            Config::path(),
            Some(PathBuf::from("/xdg/conf/rediff/config.toml")),
            "XDG_CONFIG_HOME wins when set"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", "/home/u");
        assert_eq!(
            Config::path(),
            Some(PathBuf::from("/home/u/.config/rediff/config.toml")),
            "falls back to HOME/.config"
        );

        std::env::remove_var("HOME");
        assert_eq!(Config::path(), None, "neither set → no path");

        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn write_theme_refuses_to_clobber_a_malformed_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original = "# prefs\nmode = \"split\"\ntheme = \"dark\nbroken";
        std::fs::write(&path, original).unwrap();

        // A malformed existing file is an error, and the file is left untouched
        // rather than overwritten with a theme-only document.
        assert!(super::write_theme(&path, "Dracula").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
}
