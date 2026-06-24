use super::*;

fn commit(short: &str, summary: &str) -> CommitInfo {
    CommitInfo {
        id: format!("{short}0000"),
        short: short.into(),
        summary: summary.into(),
        author: "A".into(),
        date: "2026-06-21".into(),
    }
}

#[test]
fn click_maps_through_sidebar_scroll_window() {
    use crate::model::{Changeset, DiffFile, FileStatus};
    use ratatui::layout::Rect;
    let files = (0..12)
        .map(|i| DiffFile::stub(format!("f{i}.rs"), None, FileStatus::Modified, false, None))
        .collect();
    let cs = Changeset {
        source: "wt".into(),
        files,
    };
    let mut app = App::new(&cs);
    app.grouping = sidebar::Grouping::Flat; // this exercises flat positional mapping
                                            // Simulate a scrolled sidebar window: rows 4..9 are visible on screen.
    app.sidebar_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 6,
    };
    app.sidebar_top = 4;
    app.sidebar_visible = 5;
    // Clicking the third visible row (y=2) selects file 4 + 2 = 6, not 2.
    assert!(app.click(1, 2));
    assert_eq!(app.state().selected, 6, "click honors the scroll offset");
    // A click below the visible rows is ignored.
    assert!(
        !app.click(1, 5),
        "click past the last visible row does nothing"
    );
}

#[test]
fn commit_text_filter_ranks_summary_and_short() {
    let commits = vec![
        commit("aaa1", "fix parser bug"),
        commit("bbb2", "add cli flag"),
        commit("ccc3", "refactor parser"),
    ];
    // Empty query keeps order.
    assert_eq!(filter_commits_text(&commits, ""), vec![0, 1, 2]);
    // "parser" matches the two parser commits, not the cli one.
    let m = filter_commits_text(&commits, "parser");
    assert!(m.contains(&0) && m.contains(&2));
    assert!(!m.contains(&1), "non-matching commit filtered out");
    // Short-sha is part of the haystack.
    assert_eq!(filter_commits_text(&commits, "bbb2"), vec![1]);
}

/// A tiny changeset with the given file paths (undiffed stubs).
fn paths_cs(paths: &[&str]) -> Changeset {
    use crate::model::{DiffFile, FileStatus};
    Changeset {
        source: "wt".into(),
        files: paths
            .iter()
            .map(|p| DiffFile::stub((*p).to_string(), None, FileStatus::Modified, false, None))
            .collect(),
    }
}

/// A commit palette over the given commits, unscoped, carrying `query`.
fn commit_palette(commits: Vec<CommitInfo>, query: &str) -> Palette {
    Palette {
        kind: PaletteKind::Commits {
            commits,
            scoped_path: None,
            truncated: false,
        },
        query: query.into(),
        matches: Vec::new(),
        selected: 0,
        mode_hint: "unset",
    }
}

#[test]
fn refresh_commit_mode_ignores_non_commit_palette() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // A file-jump palette is left untouched (early return).
    let mut p = Palette {
        kind: PaletteKind::Files,
        query: "abcd".into(),
        matches: vec![7],
        selected: 0,
        mode_hint: "untouched",
    };
    app.refresh_commit_mode(&mut p);
    assert_eq!(
        p.mode_hint, "untouched",
        "non-commit palette is not retyped"
    );
    assert_eq!(p.matches, vec![7], "matches are left as-is");
}

#[test]
fn refresh_commit_mode_empty_query_is_summary() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    let commits = vec![commit("aaa1", "first"), commit("bbb2", "second")];
    let mut p = commit_palette(commits, "");
    app.refresh_commit_mode(&mut p);
    assert_eq!(p.mode_hint, "summary");
    // An empty query keeps changeset order.
    assert_eq!(p.matches, vec![0, 1]);
}

#[test]
fn refresh_commit_mode_scoped_list_is_file_history() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // A file-scoped list stays file history regardless of the (non-empty) query.
    let mut p = Palette {
        kind: PaletteKind::Commits {
            commits: vec![commit("aaa1", "touch a"), commit("bbb2", "other")],
            scoped_path: Some("a.rs".into()),
            truncated: false,
        },
        query: "touch".into(),
        matches: Vec::new(),
        selected: 0,
        mode_hint: "unset",
    };
    app.refresh_commit_mode(&mut p);
    assert_eq!(p.mode_hint, "file history");
    // Recomputed over the scoped summaries: "touch" matches commit 0 only.
    assert_eq!(p.matches, vec![0]);
}

#[test]
fn refresh_commit_mode_hex_prefix_is_sha() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // `commit("aaa1", ..)` has id "aaa10000"; only it shares the "aaa1" prefix.
    let commits = vec![commit("aaa1", "first"), commit("bbb2", "second")];
    let mut p = commit_palette(commits, "aaa1");
    // Stale selection past the new match count is clamped.
    p.selected = 5;
    app.refresh_commit_mode(&mut p);
    assert_eq!(p.mode_hint, "sha");
    assert_eq!(
        p.matches,
        vec![0],
        "only the prefix-matching commit survives"
    );
    assert_eq!(p.selected, 0, "selection clamped to the last match");
}

#[test]
fn refresh_commit_mode_unknown_text_is_summary() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // "topic" is non-hex and not a known path -> fuzzy summary fallback.
    let commits = vec![commit("aaa1", "add topic"), commit("bbb2", "other work")];
    let mut p = commit_palette(commits, "topic");
    app.refresh_commit_mode(&mut p);
    assert_eq!(p.mode_hint, "summary");
    assert_eq!(p.matches, vec![0], "fuzzy match over the summaries");
}

#[test]
fn refresh_commit_mode_exact_path_rescopes_to_history() {
    // An exactly-typed known path re-scopes the list to that file's history,
    // clearing the query (like `F`). Needs a real repo to enumerate.
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cs = paths_cs(&["Cargo.toml"]);
    let mut app = App::with_launch(
        &cs,
        crate::model::LayoutMode::Stack,
        ThemeName::Dark,
        Some(dir),
        ViewKind::Local,
        true,
        None,
        None,
    );
    let mut p = commit_palette(vec![commit("aaa1", "unrelated")], "Cargo.toml");
    app.refresh_commit_mode(&mut p);
    assert_eq!(p.mode_hint, "file history");
    assert!(p.query.is_empty(), "query is cleared on re-scope");
    match &p.kind {
        PaletteKind::Commits {
            scoped_path,
            commits,
            ..
        } => {
            assert_eq!(scoped_path.as_deref(), Some("Cargo.toml"));
            assert_eq!(
                p.matches.len(),
                commits.len(),
                "every commit in the scoped history is shown"
            );
            assert!(!commits.is_empty(), "Cargo.toml has commit history");
        }
        PaletteKind::Files => panic!("expected a re-scoped commit list"),
    }
}

// ---- theme picker -------------------------------------------------------

#[test]
fn theme_picker_commit_returns_some_only_when_open() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // Nothing open → commit is a no-op returning None.
    assert!(app.theme_picker_commit().is_none(), "no picker → None");
    // Open the picker, then commit → returns the active theme and closes it.
    app.open_theme_picker();
    assert!(app.theme_picker_open());
    let committed = app.theme_picker_commit();
    assert_eq!(
        committed,
        Some(app.theme.name),
        "commit yields active theme"
    );
    assert!(!app.theme_picker_open(), "commit closes the picker");
}

#[test]
fn theme_picker_next_advances_and_wraps() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // No picker open → early return, no panic.
    app.theme_picker_next();
    assert!(!app.theme_picker_open());

    app.open_theme_picker();
    let count = app.theme_picker_count();
    assert!(count > 1, "the dark tab has several themes");
    // Start from a known position.
    if let Some(Overlay::ThemePicker(p)) = app.mode.overlay_mut() {
        p.selected = 0;
    }
    app.theme_picker_next();
    assert_eq!(app.theme_picker().map(|p| p.selected), Some(1), "advances");
    // Walk to the last theme, then once more to wrap back to 0.
    for _ in 1..count {
        app.theme_picker_next();
    }
    assert_eq!(
        app.theme_picker().map(|p| p.selected),
        Some(0),
        "wraps past the end"
    );
}

#[test]
fn theme_picker_toggle_tab_switches_and_clamps() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // No picker → toggle is a no-op (None branch).
    app.theme_picker_toggle_tab();
    assert!(!app.theme_picker_open());

    app.open_theme_picker();
    // Default theme is dark, so the dark tab is shown first.
    assert_eq!(app.theme_picker().map(|p| p.dark_tab), Some(true));
    // Force a stale, out-of-range selection so the toggle has to clamp it.
    if let Some(Overlay::ThemePicker(p)) = app.mode.overlay_mut() {
        p.selected = 9999;
    }
    app.theme_picker_toggle_tab();
    assert_eq!(
        app.theme_picker().map(|p| p.dark_tab),
        Some(false),
        "switched to the light tab"
    );
    let light_len = theme::themes_by_brightness(false).len();
    assert!(
        app.theme_picker().map(|p| p.selected).unwrap() < light_len,
        "selection clamped into the light tab's bounds"
    );
    // Toggle back to dark.
    app.theme_picker_toggle_tab();
    assert_eq!(app.theme_picker().map(|p| p.dark_tab), Some(true));
}

// ---- click --------------------------------------------------------------

#[test]
fn click_on_directory_header_toggles_its_fold() {
    use ratatui::layout::Rect;
    // Files under a single `src/` directory; default grouping is ByDir, so the
    // sidebar rows are: Dir("src"), File(0), File(1).
    let cs = paths_cs(&["src/a.rs", "src/b.rs"]);
    let mut app = App::new(&cs);
    app.sidebar_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 5,
    };
    app.sidebar_top = 0;
    app.sidebar_visible = 3;
    // A click outside the sidebar columns (a "stream" click) hits nothing.
    assert!(
        !app.click(40, 0),
        "click past the sidebar width does nothing"
    );
    // Clicking the directory header row folds the directory.
    assert!(app.click(1, 0), "click on the dir header is handled");
    assert!(
        app.state().collapsed.contains("src"),
        "the directory is now folded"
    );
    assert_eq!(app.focus(), Focus::Sidebar, "a sidebar click focuses it");
    // Clicking it again unfolds it.
    assert!(app.click(1, 0));
    assert!(
        !app.state().collapsed.contains("src"),
        "the directory is unfolded again"
    );
}

// ---- palette accessors / movement ---------------------------------------

#[test]
fn take_palette_handles_each_overlay_state() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // No overlay → None, and the slot stays empty.
    assert!(app.take_palette().is_none(), "nothing to take");
    assert!(app.mode.overlay().is_none());
    // A non-palette overlay (help) → None, and it is put back untouched.
    app.toggle_help();
    assert!(app.take_palette().is_none(), "help is not a palette");
    assert!(app.help_open(), "the help overlay is restored");
    // A palette → taken out, leaving the slot empty.
    app.palette_close();
    app.open_palette();
    assert!(app.take_palette().is_some(), "the palette is taken");
    assert!(app.mode.overlay().is_none(), "slot emptied after take");
}

#[test]
fn palette_move_clamps_both_ends_and_ignores_no_palette() {
    let cs = paths_cs(&["a.rs", "b.rs", "c.rs"]);
    let mut app = App::new(&cs);
    // No palette open → palette_mut is None, nothing happens.
    app.palette_move(1);
    assert!(!app.palette_open());

    app.open_palette();
    // Empty query matches all three files.
    assert_eq!(app.palette().map(|p| p.matches.len()), Some(3));
    // Clamp at the bottom.
    app.palette_move(100);
    assert_eq!(
        app.palette().map(|p| p.selected),
        Some(2),
        "clamped to last"
    );
    // Clamp at the top.
    app.palette_move(-100);
    assert_eq!(
        app.palette().map(|p| p.selected),
        Some(0),
        "clamped to first"
    );
    // Empty matches → early return without touching selection.
    app.palette_mut().unwrap().matches.clear();
    app.palette_move(1);
    assert_eq!(
        app.palette().map(|p| p.selected),
        Some(0),
        "no move on empty matches"
    );
}

#[test]
fn palette_pick_jumps_only_for_valid_index() {
    let cs = paths_cs(&["a.rs", "b.rs"]);
    let mut app = App::new(&cs);
    // No palette → no-op.
    app.palette_pick(0);
    assert!(!app.palette_open());

    app.open_palette();
    // Out-of-range pick is ignored; the palette stays open.
    app.palette_pick(5);
    assert!(app.palette_open(), "out-of-range pick is a no-op");
    // A valid pick selects that match, confirms, and closes the palette.
    let target = app.palette().unwrap().matches[1];
    app.palette_pick(1);
    assert!(!app.palette_open(), "a valid pick closes the palette");
    assert_eq!(
        app.state().selected,
        target,
        "the picked file is now selected"
    );
}

// ---- range exclusion / help ---------------------------------------------

#[test]
fn range_exclusion_only_excludes_for_a_range_view() {
    // A throwaway two-commit repo so the range resolves regardless of the crate
    // repo's own (squashable) commit count — see testutil::multi_commit_repo.
    let repo = crate::testutil::multi_commit_repo();
    let dir = repo.path().to_path_buf();
    let cs = paths_cs(&["a.rs"]);
    // A non-range (Local) view → no commits are excluded.
    let local = App::with_launch(
        &cs,
        crate::model::LayoutMode::Stack,
        ThemeName::Dark,
        Some(dir.clone()),
        ViewKind::Local,
        true,
        None,
        None,
    );
    assert!(
        local.range_exclusion(&dir).is_empty(),
        "non-range views exclude nothing"
    );
    // A range view → the range's own commits are returned.
    let ranged = App::with_launch(
        &cs,
        crate::model::LayoutMode::Stack,
        ThemeName::Dark,
        Some(dir.clone()),
        ViewKind::Range {
            base: "HEAD~1".into(),
            target: "HEAD".into(),
        },
        true,
        None,
        None,
    );
    assert!(
        !ranged.range_exclusion(&dir).is_empty(),
        "HEAD~1..HEAD has at least one commit to exclude"
    );
}

// ---- commit-message popup -----------------------------------------------

/// An app over the crate's own repo (a Local home view), so the by-sha message
/// fetch and `open_commit` have a live repository to read.
fn repo_app() -> App {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cs = paths_cs(&["a.rs"]);
    App::with_launch(
        &cs,
        crate::model::LayoutMode::Stack,
        ThemeName::Dark,
        Some(dir),
        ViewKind::Local,
        true,
        None,
        None,
    )
}

#[test]
fn open_commit_message_fetches_body_and_dismiss_restores_picker() {
    let mut app = repo_app();
    // Summon the popup over a commit picker: the picker is stashed and restored.
    app.mode.push_overlay(Overlay::Palette(commit_palette(
        vec![commit("aaa1", "unrelated")],
        "",
    )));
    app.open_commit_message("HEAD");
    assert!(app.commit_msg_open(), "popup opened for HEAD");
    assert!(
        !app.commit_msg().unwrap().msg.body.is_empty(),
        "the full body was fetched"
    );
    app.commit_msg_dismiss();
    assert!(!app.commit_msg_open(), "dismiss closes the popup");
    // Dismiss pops the popup, revealing the picker that was stacked beneath it.
    assert!(app.palette_open(), "the picker is restored beneath it");
}

#[test]
fn open_commit_message_with_an_unresolvable_rev_is_noop() {
    // A repo is present but the rev doesn't resolve → the fetch fails and no
    // popup is pushed (the `Err` arm).
    let mut app = repo_app();
    app.open_commit_message("zzz-no-such-rev-zzz");
    assert!(!app.commit_msg_open(), "an unresolvable rev opens nothing");
}

#[test]
fn open_commit_message_without_repo_is_noop() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs); // no repo_dir
    app.open_commit_message("HEAD");
    assert!(!app.commit_msg_open(), "no repo → nothing opens");
}

#[test]
fn commit_msg_confirm_switches_to_the_commit() {
    let mut app = repo_app();
    app.open_commit_message("HEAD");
    assert!(app.commit_msg_open());
    app.commit_msg_confirm();
    assert!(!app.commit_msg_open(), "confirm closes the popup");
    assert_eq!(app.session.cursor, 1, "a commit view was pushed");
}

#[test]
fn active_context_resolves_every_surface_and_binds_its_table() {
    use crate::tui::keymap as k;
    // Binding tables are consts (inlined per use), so identity is by rendered
    // hint, not pointer.
    let hint = |b: &[k::Binding]| k::to_hint(b);
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);

    // Normal base → the focused pane's table.
    assert_eq!(app.active_context(), InputContext::Normal);
    let focus_hint = hint(app.status_bindings());
    assert!(
        focus_hint == hint(k::BIND_STREAM) || focus_hint == hint(k::BIND_SIDEBAR),
        "normal context binds a pane table: {focus_hint}"
    );

    // Peek base (no overlay) → the peek's per-mode table.
    app.open_peek_preview();
    assert_eq!(app.active_context(), InputContext::Peek);
    assert_eq!(hint(app.status_bindings()), hint(k::BIND_PEEK_CONTENT));
    app.peek_close();

    // File palette.
    app.open_palette();
    assert_eq!(app.active_context(), InputContext::Palette);
    assert_eq!(hint(app.status_bindings()), hint(k::BIND_PALETTE_FILE));
    app.palette_close();

    // Commit palette variant.
    app.mode.push_overlay(Overlay::Palette(commit_palette(
        vec![commit("aaa1", "one")],
        "",
    )));
    assert_eq!(hint(app.status_bindings()), hint(k::BIND_PALETTE_COMMIT));
    app.palette_close();

    // Theme picker.
    app.open_theme_picker();
    assert_eq!(app.active_context(), InputContext::ThemePicker);
    assert_eq!(hint(app.status_bindings()), hint(k::BIND_THEME));
    app.theme_picker_cancel();

    // Help: captures input, but the status bar renders its own hint (no table).
    app.toggle_help();
    assert_eq!(app.active_context(), InputContext::Help);
    assert!(app.status_bindings().is_empty());
    app.toggle_help();

    // Commit-message popup outranks a peek beneath it.
    app.open_peek_preview();
    app.mode.push_overlay(Overlay::CommitMessage(CommitMsg::new(
        crate::model::CommitMessage {
            sha: "s".into(),
            short: "s".into(),
            author: String::new(),
            date: String::new(),
            body: String::new(),
        },
    )));
    assert_eq!(app.active_context(), InputContext::CommitMsg);
    assert_eq!(hint(app.status_bindings()), hint(k::BIND_COMMITMSG));
}

#[test]
fn palette_confirm_with_no_matches_keeps_the_picker() {
    // A query that filters everything out has nothing to confirm; Enter must
    // keep the picker (and its query) rather than dropping it to the base view.
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    let mut p = commit_palette(vec![commit("aaa1", "one")], "zzz-no-such");
    p.matches = Vec::new(); // query matched nothing
    app.mode.push_overlay(Overlay::Palette(p));
    app.palette_confirm();
    assert!(
        app.commit_palette_open(),
        "the picker stays open on no match"
    );
}

#[test]
fn palette_confirm_failure_restores_the_picker() {
    // The synthetic commit id never enumerates, so the switch fails — the
    // picker (query, scope, selection) must come back, not vanish.
    let mut app = repo_app();
    let mut p = commit_palette(vec![commit("aaa1", "gone")], "");
    p.matches = vec![0];
    app.mode.push_overlay(Overlay::Palette(p));
    app.palette_confirm();
    assert!(
        app.commit_palette_open(),
        "a failed switch restores the picker"
    );
    assert_eq!(app.session.cursor, 0, "no view was pushed");
}

#[test]
fn commit_msg_confirm_failure_keeps_the_popup() {
    use crate::model::CommitMessage;
    // No repo dir → open_commit must fail; the popup (and whatever it stashed)
    // stays instead of stranding the user on the base view with no feedback.
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    app.mode
        .push_overlay(Overlay::CommitMessage(CommitMsg::new(CommitMessage {
            sha: "deadbeef".into(),
            short: "deadbee".into(),
            author: String::new(),
            date: String::new(),
            body: "subject".into(),
        })));
    app.commit_msg_confirm();
    assert!(app.commit_msg_open(), "a failed switch restores the popup");
    assert_eq!(app.session.cursor, 0, "no view was pushed");
}

#[test]
fn overlays_stack_and_unwind_without_dropping_layers() {
    // The stack's whole purpose: a third overlay opened over the popup pushes,
    // it does not overwrite — so unwinding reveals each layer in turn.
    let mut app = repo_app();
    app.mode.push_overlay(Overlay::Palette(commit_palette(
        vec![commit("aaa1", "x")],
        "",
    )));
    app.open_commit_message("HEAD");
    assert!(app.commit_msg_open(), "popup over the picker");
    // Simulate a future opener firing while the popup is up.
    app.mode.push_overlay(Overlay::Help);
    assert!(app.help_open(), "help is now the active overlay");
    // Unwind: help → popup → picker, nothing dropped.
    app.toggle_help();
    assert!(app.commit_msg_open(), "popup revealed beneath help");
    app.commit_msg_dismiss();
    assert!(
        app.commit_palette_open(),
        "picker revealed beneath the popup"
    );
}

#[test]
fn commit_msg_confirm_lands_on_the_stashed_pickers_file() {
    // Confirming from the popup must land on the stashed file-history picker's
    // scoped file, exactly like confirming in the picker itself does.
    let mut app = repo_app();
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let exclude = std::collections::HashSet::new();
    let Ok((commits, _)) =
        crate::git::enumerate_commits(&dir, "HEAD", 1, Some("Cargo.toml"), &exclude)
    else {
        return;
    };
    let Some(c) = commits.first() else { return };
    // A file-history picker on the stack, with the popup summoned over it.
    app.mode.push_overlay(Overlay::Palette(Palette {
        kind: PaletteKind::Commits {
            commits: Vec::new(),
            scoped_path: Some("Cargo.toml".into()),
            truncated: false,
        },
        query: String::new(),
        matches: Vec::new(),
        selected: 0,
        mode_hint: "",
    }));
    app.open_commit_message(&c.id);
    app.commit_msg_confirm();
    assert_eq!(app.session.cursor, 1, "switched to the commit view");
    let sel = app.state().selected;
    assert_eq!(
        app.cs().files.get(sel).map(|f| f.path.clone()),
        Some("Cargo.toml".to_string()),
        "landed on the scoped file"
    );
}

#[test]
fn palette_tab_opens_highlighted_real_commit_then_dismiss_restores() {
    let mut app = repo_app();
    app.open_commit_palette();
    assert!(app.commit_palette_open(), "the commit picker is open");
    // Tab on the highlighted (real) commit opens its message over the picker.
    app.palette_open_highlighted_message();
    assert!(app.commit_msg_open(), "Tab opened the message popup");
    app.commit_msg_dismiss();
    assert!(
        app.commit_palette_open(),
        "dismiss restores the commit picker"
    );
}

#[test]
fn palette_tab_is_noop_for_the_file_palette() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    app.open_palette(); // a file palette, not commits
    app.palette_open_highlighted_message();
    assert!(
        !app.commit_msg_open(),
        "Tab does nothing in the file palette"
    );
    assert!(app.palette_open(), "the file palette stays open");
}

#[test]
fn commit_msg_ops_are_noops_without_a_popup() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    // Every popup op short-circuits when no commit-message popup is open.
    app.commit_msg_scroll(1);
    app.commit_msg_confirm();
    app.commit_msg_dismiss();
    assert!(!app.commit_msg_open());
}

#[test]
fn commit_msg_scroll_stops_a_page_short_of_the_bottom() {
    use crate::model::CommitMessage;
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    let body = (1..=10)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.mode
        .push_overlay(Overlay::CommitMessage(CommitMsg::new(CommitMessage {
            sha: "x".into(),
            short: "x".into(),
            author: "a".into(),
            date: "d".into(),
            body,
        })));
    // A 4-row viewport over 10 lines: scrolling stops at line 6 (10 − 4) so the
    // last page stays full, not at line 9.
    app.commit_msg_viewport_h = 4;
    app.commit_msg_scroll(10_000);
    assert_eq!(
        app.commit_msg().unwrap().scroll,
        6,
        "stops a page short so the last screen stays full"
    );
    app.commit_msg_scroll(-10_000);
    assert_eq!(app.commit_msg().unwrap().scroll, 0, "clamps to the top");
}

#[test]
fn toggle_help_opens_then_closes() {
    let cs = paths_cs(&["a.rs"]);
    let mut app = App::new(&cs);
    assert!(!app.help_open());
    app.toggle_help();
    assert!(app.help_open(), "first toggle opens help");
    app.toggle_help();
    assert!(!app.help_open(), "second toggle closes help");
}
