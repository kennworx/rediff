//! rediff — a fast Rust TUI git-diff viewer.
//!
//! Launches the interactive TUI when stdout is a terminal; otherwise prints
//! the changeset as unified diff text (so pipes and redirects still work).

use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;

use rediff::cli::{Cli, Command};
use rediff::config::{self, Config};
use rediff::model::LayoutMode;
use rediff::tui::{ThemeName, ViewKind};
use rediff::{git, pager, render, tui};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // pager/external are non-interactive stdin→stdout filters (git/lazygit
    // pagers): they never open a repo or the TUI, so handle them first.
    if let Some(result) = run_filter_command(&cli) {
        return result;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolved = cli.resolve(&cwd);
    let cfg = Config::load();

    // Precedence: CLI flag > config file > pick by terminal width at startup
    // (`None` defers that choice to `tui::run`).
    let mode: Option<LayoutMode> = resolved
        .mode
        .as_deref()
        .and_then(config::parse_mode)
        .or_else(|| cfg.layout_mode());
    let theme = resolved
        .theme
        .as_deref()
        .or(cfg.theme.as_deref())
        .map(ThemeName::parse)
        .unwrap_or_default();

    if std::io::stdout().is_terminal() {
        // The TUI enumerates the file list instantly and streams the diffs in;
        // no synchronous full load up front.
        let (kind, base) = ViewKind::launch_for(&resolved.req);
        tui::run(
            &resolved.req,
            &resolved.filters,
            mode,
            theme,
            resolved.repo_dir.clone(),
            kind,
            resolved.review,
            base,
        )?;
    } else {
        // Pipes/redirects get the full diff synchronously.
        let mut changeset = git::load(&resolved.repo_dir, &resolved.req)?;
        git::apply_path_filter(&mut changeset, &resolved.filters);
        print!("{}", render::to_unified_string(&changeset));
    }
    Ok(())
}

/// Dispatch the non-interactive filter subcommands (`pager`, `external`).
/// Returns `Some(result)` when `cli` selected one — so `main` returns it
/// immediately — or `None` for the normal repo/TUI path.
fn run_filter_command(cli: &Cli) -> Option<anyhow::Result<()>> {
    match &cli.command {
        // `pager` is a stdin→stdout filter (a git/lazygit pager).
        Some(Command::Pager { theme }) => Some(pager::run(filter_theme(theme.as_deref()))),
        // `external` is the GIT_EXTERNAL_DIFF per-file renderer.
        Some(Command::External { theme, args }) => {
            Some(pager::external(args, filter_theme(theme.as_deref())))
        }
        _ => None,
    }
}

/// Resolve a filter command's theme: explicit `--theme` flag, else the config
/// file, else the default.
fn filter_theme(flag: Option<&str>) -> ThemeName {
    let cfg = Config::load();
    flag.or(cfg.theme.as_deref())
        .map(ThemeName::parse)
        .unwrap_or_default()
}
