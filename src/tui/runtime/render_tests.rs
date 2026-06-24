//! Rendering, scrolling, and app-navigation tests: frame painting, horizontal
//! pan/clamp, sidebar visibility, the theme picker, the fuzzy palette, and help.

use super::keys::{handle_key, BIG_STEP};
use crate::diff::compute_hunks;
use crate::model::{Changeset, DiffFile, FileStatus, LayoutMode, Stats};
use crate::tui::app::{App, Focus};
use crate::tui::theme::ThemeName;
use crate::tui::ui;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use std::fmt::Write as _;

fn file(path: &str, old: &str, new: &str, status: FileStatus) -> DiffFile {
    let (hunks, additions, deletions) = compute_hunks(old, new);
    DiffFile {
        path: path.into(),
        previous_path: None,
        status,
        staged: false,
        hunks,
        stats: Stats {
            additions,
            deletions,
        },
        language: None,
        is_binary: false,
        old_text: (!old.is_empty()).then(|| old.to_string()),
        new_text: (!new.is_empty()).then(|| new.to_string()),
        diffed: true,
    }
}

fn sample() -> Changeset {
    Changeset {
        source: "working tree".into(),
        files: vec![
            file(
                "src/auth.rs",
                "fn login() {\n    ok()\n}\n",
                "fn login() {\n    check()\n    ok()\n}\n",
                FileStatus::Modified,
            ),
            file("README.md", "", "hello\n", FileStatus::Untracked),
        ],
    }
}

/// A changeset big enough that the stream actually scrolls.
fn big_sample() -> Changeset {
    let files = (0..5)
        .map(|i| {
            let mut old = String::new();
            for n in 0..25 {
                writeln!(old, "line {n}").unwrap();
            }
            let mut new = String::new();
            for n in 0..25 {
                writeln!(new, "line {n}{}", if n == 5 { " X" } else { "" }).unwrap();
            }
            file(&format!("src/file{i}.rs"), &old, &new, FileStatus::Modified)
        })
        .collect();
    Changeset {
        source: "wt".into(),
        files,
    }
}

fn render_to_string(w: u16, h: u16) -> String {
    let cs = sample();
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn paint_does_not_mutate_app() {
    let cs = sample();
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(74, 16)).unwrap();
    // A full draw reconciles the geometry-derived state onto `app`.
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let snap = (
        app.viewport_h,
        app.viewport_w,
        app.peek_viewport_h,
        app.sidebar_area,
        app.sidebar_top,
        app.sidebar_visible,
        app.sidebar_height,
        app.state().scroll,
        app.state().h_scroll,
        app.state().selected,
        app.plan().rows.len(),
    );
    // The paint pass is pure: measuring + painting from `&app` must change no
    // observable state.
    term.draw(|f| {
        let g = ui::measure(&app, f.area());
        ui::paint(f, &app, &g);
    })
    .unwrap();
    let after = (
        app.viewport_h,
        app.viewport_w,
        app.peek_viewport_h,
        app.sidebar_area,
        app.sidebar_top,
        app.sidebar_visible,
        app.sidebar_height,
        app.state().scroll,
        app.state().h_scroll,
        app.state().selected,
        app.plan().rows.len(),
    );
    assert_eq!(snap, after, "paint must not mutate App");
}

#[test]
fn renders_review_frame() {
    let out = render_to_string(74, 16);
    println!("\n{out}");
    assert!(out.contains("auth.rs"), "sidebar/header shows the file");
    assert!(out.contains('+'), "added lines are rendered");
    assert!(out.contains("hunk"), "stream status hint present");
}

/// Render one frame. These demos exercise the render path and assert on
/// theme-derived, deterministic output (the syntax table, the split divider,
/// diff text) — not on the async highlighter, whose worker builds a full engine
/// and can't be awaited reliably under parallel/CI load (and isn't what these
/// render tests are about). The worker → `drain` → render path is covered by the
/// PTY event-loop test instead.
fn render_once(app: &mut App, term: &mut Terminal<TestBackend>) {
    term.draw(|f| ui::draw(f, app)).unwrap();
}

#[test]
fn renders_highlighted_frame_demo() {
    use ratatui::style::Color;
    let cs = Changeset {
        source: "working tree".into(),
        files: vec![{
            let old = "fn main() {\n    let x = 1;\n}\n";
            let new = "fn main() {\n    // greet\n    let name = \"world\";\n    println!(\"hi {name}\");\n}\n";
            let mut f = file("src/main.rs", old, new, FileStatus::Modified);
            f.language = Some("rust".into());
            f
        }],
    };
    let mut app = App::new(&cs);

    let mut term = Terminal::new(TestBackend::new(72, 12)).unwrap();
    render_once(&mut app, &mut term);

    // Dump the frame as truecolor ANSI so the colors are visible.
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..12u16 {
        for x in 0..72u16 {
            let cell = &buf[(x, y)];
            if let Color::Rgb(r, g, b) = cell.fg {
                write!(out, "\x1b[38;2;{r};{g};{b}m{}", cell.symbol()).unwrap();
            } else {
                write!(out, "\x1b[0m{}", cell.symbol()).unwrap();
            }
        }
        out.push_str("\x1b[0m\n");
    }
    println!("\n{out}");

    // The theme's syntax table resolves capture index 9 (keywords) to a color
    // distinct from default text — this is what `ui::resolve` paints `fn` with
    // once the (async) highlight lands. Asserting the table keeps the test
    // deterministic; the resolve-to-cell render path is covered by the ui tests.
    let kw = app.syntax[9];
    let kw = Color::Rgb(kw.0, kw.1, kw.2);
    assert_ne!(
        kw, app.theme.context,
        "keyword color is distinct from default text"
    );
}

#[test]
fn renders_split_layout_demo() {
    let cs = Changeset {
        source: "working tree".into(),
        files: vec![{
            let old = "fn main() {\n    old_call();\n}\n";
            let new = "fn main() {\n    new_call();\n    extra();\n}\n";
            let mut f = file("src/main.rs", old, new, FileStatus::Modified);
            f.language = Some("rust".into());
            f
        }],
    };
    let mut app = App::with_mode(&cs, LayoutMode::Split);
    let mut term = Terminal::new(TestBackend::new(90, 10)).unwrap();
    render_once(&mut app, &mut term);
    assert!(app.is_split(), "split mode forces side-by-side");

    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..10u16 {
        for x in 0..90u16 {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    println!("\n{out}");
    assert!(out.contains('│'), "split has a column divider");
    assert!(
        out.contains("old_call") && out.contains("new_call"),
        "both sides shown"
    );
}

#[test]
fn sparse_digit_mapping_spreads_evenly() {
    use crate::tui::sidebar::{digit_to_offset, offset_to_digit};
    // 17 visible files → digit 1 hits offset 0, digit 9 hits offset 16.
    assert_eq!(digit_to_offset(1, 17), 0);
    assert_eq!(digit_to_offset(9, 17), 16);
    assert_eq!(digit_to_offset(5, 17), 8);
    assert_eq!(offset_to_digit(0, 17), Some(1));
    assert_eq!(offset_to_digit(16, 17), Some(9));
    // small set maps 1:1
    assert_eq!(digit_to_offset(2, 3), 1);
}

#[test]
fn grouped_sidebar_renders_dir_lines_and_basenames() {
    // Hand-built (bypasses the enumerate sort), so list the files path-sorted.
    let cs = Changeset {
        source: "working tree".into(),
        files: vec![
            file("README.md", "", "x\n", FileStatus::Modified),
            file("src/a.rs", "", "x\n", FileStatus::Modified),
            file("src/b.rs", "", "x\n", FileStatus::Modified),
        ],
    };
    let mut app = App::with_mode(&cs, crate::model::LayoutMode::Stack);
    // Grouped by directory is the default — no toggle needed.
    let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer();
    // Only the sidebar columns — the right pane (the diff body) shows full
    // paths in its file headers, which is not what this test is about.
    let sb_w = app.sidebar_area.width;
    let mut text = String::new();
    for y in 0..buf.area.height {
        for x in 0..sb_w.min(buf.area.width) {
            text.push_str(buf[(x, y)].symbol());
        }
        text.push('\n');
    }
    assert!(text.contains("./"), "root directory line shown: {text}");
    assert!(text.contains("src/"), "src directory line shown: {text}");
    assert!(
        text.contains("a.rs") && text.contains("b.rs"),
        "basenames shown: {text}"
    );
    assert!(
        !text.contains("src/a.rs"),
        "grouped file rows show basenames, not full paths: {text}"
    );
}

#[test]
fn horizontal_scroll_is_bounded_by_content_width() {
    // One file with a long line; pan right past the end and confirm it stops.
    let long: String = "x".repeat(300);
    let cs = Changeset {
        source: "wt".into(),
        files: vec![file(
            "a.rs",
            "short\n",
            &format!("{long}\n"),
            FileStatus::Modified,
        )],
    };
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();

    // Pan far right; it must clamp so the line's tail can't go past the edge.
    for _ in 0..200 {
        app.h_scroll_by(8);
    }
    let max = app.plan().content_w.saturating_sub(app.viewport_w);
    assert!(
        app.state().h_scroll <= max,
        "h_scroll {} exceeds max {}",
        app.state().h_scroll,
        max
    );
    assert!(app.state().h_scroll > 0, "a long line is still pannable");
    // Content stays on screen: at most max columns are scrolled away.
    assert!(app.state().h_scroll + app.viewport_w >= app.plan().content_w);
}

#[test]
fn horizontal_scroll_keeps_gutter_pins_content() {
    // A long line with a marker at the start and end; the line number stays
    // visible while the start scrolls away and the end comes into view.
    let new = format!("AAAA {} ZZZZ\n", "m".repeat(120));
    let cs = Changeset {
        source: "wt".into(),
        files: vec![file("a.rs", "old\n", &new, FileStatus::Modified)],
    };
    let mut app = App::new(&cs);
    let (w, h) = (90u16, 12u16);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();

    for _ in 0..300 {
        app.h_scroll_by(8);
    }
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    assert!(
        out.contains("ZZZZ"),
        "end of the line is reachable by panning"
    );
    assert!(!out.contains("AAAA"), "start of the line scrolled away");
    // The added line's number (1) and '+' sign stay pinned, with panned
    // content ('m') immediately after the gutter.
    assert!(
        out.contains("1 +m"),
        "gutter pinned ahead of panned content: {out}"
    );
}

#[test]
fn split_horizontal_scroll_pans_within_columns() {
    let mut long = String::new();
    for i in 0..40 {
        write!(long, "token{i} ").unwrap();
    }
    let cs = Changeset {
        source: "wt".into(),
        files: vec![file(
            "a.rs",
            "old\n",
            &format!("{long}\n"),
            FileStatus::Modified,
        )],
    };
    let mut app = App::with_mode(&cs, LayoutMode::Split);
    let mut term = Terminal::new(TestBackend::new(100, 12)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(app.is_split());

    for _ in 0..200 {
        app.h_scroll_by(8);
    }
    let col_w = app.viewport_w.saturating_sub(1) / 2;
    let max = app.plan().content_w.saturating_sub(col_w);
    assert!(
        app.state().h_scroll <= max,
        "split h_scroll {} exceeds max {}",
        app.state().h_scroll,
        max
    );
    assert!(app.state().h_scroll > 0, "long line pans in split too");

    // The column divider is still on screen (layout not scrolled off).
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..12 {
        for x in 0..100 {
            out.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(
        out.contains('│'),
        "divider stays visible when panned in split"
    );
}

#[test]
fn viewed_collapses_file_in_plan() {
    let cs = sample();
    let mut app = App::new(&cs);
    let before = app.plan().rows.len();
    app.toggle_viewed(); // file 0 (auth.rs) at top of viewport
    assert!(app.state().viewed[0]);
    assert!(
        app.plan().rows.len() < before,
        "viewed file should collapse its hunks"
    );
    assert!(app.next_unviewed(), "README.md is still unviewed");
    assert_eq!(app.current_file(), 1);
}

#[test]
fn palette_filters_and_jumps() {
    let cs = sample();
    let mut app = App::new(&cs);
    app.open_palette();
    for c in "read".chars() {
        app.palette_input(c);
    }
    let p = app.palette().unwrap();
    assert!(
        p.matches
            .first()
            .is_some_and(|&i| cs.files[i].path.contains("README")),
        "README should be the top match for 'read'"
    );
    app.palette_confirm();
    assert!(!app.palette_open());
    assert_eq!(app.current_file(), 1, "jumped to README");
}

#[test]
fn theme_picker_previews_and_commits() {
    let cs = sample();
    let mut app = App::with_options(
        &cs,
        crate::model::LayoutMode::Stack,
        crate::tui::theme::ThemeName::Dark,
    );
    assert!(app.theme.dark);
    let original = app.theme.name;

    // `t` opens the picker on the dark tab (the active theme is dark).
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::NONE);
    assert!(app.theme_picker_open());

    // The picker grid renders without panic while open.
    let mut term = Terminal::new(TestBackend::new(90, 20)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();

    // Tab switches to the light tab and live-previews a light theme.
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert!(
        !app.theme.dark,
        "switching to the light tab previews a light theme"
    );

    // Esc rolls back to the theme active when the picker opened.
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.theme_picker_open());
    assert_eq!(
        app.theme.name, original,
        "cancel restores the original theme"
    );

    // Re-open, switch tab, and commit keeps the previewed theme. (Commit via
    // the app method, not the Enter key, so the test never writes the real
    // config — persistence is covered in `config`.)
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    let committed = app.theme_picker_commit();
    assert_eq!(committed, Some(app.theme.name));
    assert!(!app.theme_picker_open());
    assert!(!app.theme.dark, "commit keeps the previewed light theme");

    // still renders fine in the committed theme
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
}

#[test]
fn theme_picker_q_closes_and_t_advances() {
    let cs = sample();
    let mut app = App::with_options(
        &cs,
        crate::model::LayoutMode::Stack,
        crate::tui::theme::ThemeName::Dark,
    );
    let original = app.theme.name;

    // `t` opens; `t` again advances to the next theme (live preview).
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::NONE);
    assert!(app.theme_picker_open());
    assert_ne!(app.theme.name, original, "t advances to the next theme");

    // `q` closes the popup and rolls back, like Esc.
    handle_key(&mut app, KeyCode::Char('q'), KeyModifiers::NONE);
    assert!(!app.theme_picker_open(), "q closes the picker");
    assert_eq!(
        app.theme.name, original,
        "q rolls back to the original theme"
    );
    assert!(
        !app.should_quit,
        "q in the picker closes it, does not quit the app"
    );
}

#[test]
fn navigation_latency_is_fast() {
    // A large synthetic changeset: 200 files × ~50 lines each.
    let mut files = Vec::new();
    for i in 0..200 {
        let mut old = String::new();
        for n in 0..50 {
            writeln!(old, "line {n}").unwrap();
        }
        let mut new = String::new();
        for n in 0..50 {
            writeln!(new, "line {} {n}", if n == 7 { "X" } else { "" }).unwrap();
        }
        files.push(file(
            &format!("src/file{i}.rs"),
            &old,
            &new,
            FileStatus::Modified,
        ));
    }
    let cs = Changeset {
        source: "bench".into(),
        files,
    };
    let mut app = App::new(&cs);
    app.viewport_h = 40;

    // Time 1000 hunk-nav + scroll operations.
    let start = std::time::Instant::now();
    for i in 0..1000 {
        if i % 2 == 0 {
            app.next_hunk();
        } else {
            app.scroll_by(3);
        }
        if i % 100 == 0 {
            app.top();
        }
    }
    let per_op = start.elapsed().as_secs_f64() * 1000.0 / 1000.0;
    println!("nav latency: {per_op:.4} ms/op over a 200-file changeset");
    assert!(
        per_op < 1.0,
        "navigation should be well under 1ms/op (got {per_op:.4})"
    );
}

#[test]
fn toggle_sidebar_hides_panel() {
    let cs = sample();
    let mut app = App::new(&cs);
    assert!(!app.sidebar_hidden);
    app.toggle_focus(); // into sidebar
    app.toggle_sidebar(); // hide it
    assert!(app.sidebar_hidden);
    assert_eq!(
        app.focus(),
        Focus::Stream,
        "hiding the panel moves focus to the diff"
    );

    // The diff now starts at the left edge; no sidebar file-list column.
    let mut term = Terminal::new(TestBackend::new(60, 10)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..10u16 {
        for x in 0..60u16 {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    assert!(out.contains("src/auth.rs"), "diff still renders");
    assert!(!out.contains('│'), "sidebar divider is gone");

    app.toggle_sidebar();
    assert!(!app.sidebar_hidden);
}

#[test]
fn focusing_hidden_sidebar_reveals_it_temporarily() {
    let cs = sample();
    let mut app = App::new(&cs);
    app.toggle_sidebar(); // hide-mode on, focus → stream
    assert!(app.sidebar_hidden);
    assert!(!app.sidebar_shown(), "hidden while focus is on the diff");

    app.toggle_focus(); // Tab into the sidebar
    assert_eq!(app.focus(), Focus::Sidebar);
    assert!(app.sidebar_shown(), "focusing reveals the hidden sidebar");
    assert!(app.sidebar_hidden, "hide-mode is still sticky");

    app.toggle_focus(); // Tab back to the diff
    assert!(!app.sidebar_shown(), "hidden again when focus leaves it");
}

#[test]
fn sticky_header_pins_current_file() {
    let cs = big_sample();
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(70, 12)).unwrap();
    // Scroll a few rows into the first file, past its header.
    app.scroll_by(4);
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(app.state().scroll > 0);

    // The stream's top line (right of the sidebar) shows the current file.
    let buf = term.backend().buffer().clone();
    let mut top = String::new();
    for x in 34..70u16 {
        top.push_str(buf[(x, 0)].symbol());
    }
    assert!(
        top.contains("file0.rs"),
        "current file header pinned at top: {top:?}"
    );
}

#[test]
fn scrolling_updates_selected_file() {
    let cs = big_sample();
    let mut app = App::new(&cs);
    app.viewport_h = 10;
    #[expect(
        clippy::cast_possible_wrap,
        reason = "a file-start row offset in a tiny test changeset is far below isize::MAX"
    )]
    let second = app.plan().file_starts[1] as isize;
    app.scroll_by(second);
    assert_eq!(app.current_file(), 1);
    assert_eq!(
        app.state().selected,
        1,
        "scrolling the diff moves the selected file"
    );
    app.top();
    assert_eq!(
        app.state().selected,
        0,
        "scrolling back to the top reselects the first file"
    );
}

#[test]
fn selection_survives_focus_toggle() {
    let cs = big_sample(); // 5 files; last ones share the final page
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(74, 16)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();

    app.toggle_focus(); // into sidebar
    app.sidebar_move(100); // to the last file
    assert_eq!(app.state().selected, 4);

    app.focus_stream(); // switch to the diff
    app.toggle_focus(); // back to the sidebar
    assert_eq!(
        app.state().selected,
        4,
        "last-file selection survives the round trip"
    );
}

#[test]
fn help_overlay_toggles_and_renders() {
    let cs = sample();
    let mut app = App::new(&cs);
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(app.help_open());

    let (tw, th) = (90u16, 28u16);
    let mut term = Terminal::new(TestBackend::new(tw, th)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..th {
        for x in 0..tw {
            out.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(
        out.contains("pick a commit"),
        "help lists the commit picker"
    );
    assert!(out.contains("review commit"), "help lists R (promote)");

    // Any key dismisses it.
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::NONE);
    assert!(!app.help_open());
}

#[test]
fn sidebar_focus_navigates_files() {
    let cs = sample();
    let mut app = App::new(&cs);
    // focus sidebar, move down to the second file, stream should follow
    app.toggle_focus();
    assert_eq!(app.focus(), Focus::Sidebar);
    assert_eq!(app.state().selected, 0);
    app.sidebar_move(1);
    assert_eq!(app.state().selected, 1);
    assert_eq!(app.current_file(), 1, "stream jumped to the selected file");

    // render in sidebar focus shows the selection marker + focus hint
    let mut term = Terminal::new(TestBackend::new(74, 16)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..16 {
        for x in 0..74 {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    println!("\n{out}");
    assert!(out.contains("select"), "sidebar focus hint present");
    assert!(out.contains('▌'), "selection marker present");
}

#[test]
fn theme_picker_arrow_keys_navigate_grid() {
    let cs = sample();
    let mut app = App::with_options(&cs, LayoutMode::Stack, ThemeName::Dark);
    let mut term = Terminal::new(TestBackend::new(100, 24)).unwrap();
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::NONE);
    assert!(app.theme_picker_open());
    term.draw(|f| ui::draw(f, &mut app)).unwrap(); // sizes the grid

    // Arrows and hjkl all drive the grid cursor (clamped within the tab); the
    // picker stays open throughout.
    for code in [
        KeyCode::Char('j'),
        KeyCode::Down,
        KeyCode::Char('l'),
        KeyCode::Right,
        KeyCode::Char('h'),
        KeyCode::Left,
        KeyCode::Char('k'),
        KeyCode::Up,
    ] {
        handle_key(&mut app, code, KeyModifiers::NONE);
        assert!(app.theme_picker_open(), "navigation keeps the picker open");
    }
    // BackTab switches tabs (like Tab).
    handle_key(&mut app, KeyCode::BackTab, KeyModifiers::NONE);
    assert!(app.theme_picker_open());

    // Esc cancels without persisting any theme.
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.theme_picker_open(), "Esc cancels the picker");
}

#[test]
fn scroll_step_is_one_shift_is_several() {
    let long = "x".repeat(300);
    let cs = Changeset {
        source: "wt".into(),
        files: (0..12)
            .map(|i| {
                file(
                    &format!("f{i}.rs"),
                    "a\n",
                    &format!("{long}\n{long}\n"),
                    FileStatus::Modified,
                )
            })
            .collect(),
    };
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(90, 12)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();

    // Vertical: base one line, Shift several.
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(app.state().scroll, 1, "j scrolls one line");
    handle_key(&mut app, KeyCode::Char('J'), KeyModifiers::SHIFT);
    assert_eq!(
        app.state().scroll,
        1 + BIG_STEP as usize,
        "J scrolls several"
    );
    handle_key(&mut app, KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(
        app.state().scroll,
        1 + 2 * BIG_STEP as usize,
        "Shift+Down scrolls several"
    );

    // Horizontal: base one column, Shift several.
    handle_key(&mut app, KeyCode::Char('l'), KeyModifiers::NONE);
    assert_eq!(app.state().h_scroll, 1, "l pans one column");
    handle_key(&mut app, KeyCode::Char('L'), KeyModifiers::SHIFT);
    assert_eq!(
        app.state().h_scroll,
        1 + BIG_STEP as usize,
        "L pans several"
    );
    handle_key(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(
        app.state().h_scroll,
        1 + 2 * BIG_STEP as usize,
        "Shift+Right pans several"
    );
    handle_key(&mut app, KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(
        app.state().h_scroll,
        2 * BIG_STEP as usize,
        "h pans back one column"
    );
}

#[test]
fn stream_keys_cover_scroll_family() {
    let cs = big_sample();
    let mut app = App::new(&cs);
    let mut term = Terminal::new(TestBackend::new(74, 16)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert_eq!(app.focus(), Focus::Stream);

    // Vertical scroll: fast (J/K, Shift+↑↓) and single-step (j/k, ↑↓).
    handle_key(&mut app, KeyCode::Char('J'), KeyModifiers::NONE);
    assert!(app.state().scroll > 0, "J fast-scrolls down");
    handle_key(&mut app, KeyCode::Char('K'), KeyModifiers::NONE);
    assert_eq!(app.state().scroll, 0, "K fast-scrolls back up");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::SHIFT);
    assert!(app.state().scroll > 0, "Shift+Down fast-scrolls");
    handle_key(&mut app, KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(app.state().scroll, 0, "Shift+Up fast-scrolls back");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    let one = app.state().scroll;
    assert!(one > 0, "Down scrolls one line");
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::NONE);
    assert!(app.state().scroll > one, "j scrolls one more");
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.state().scroll, 0, "k/Up step back to the top");

    // Horizontal pan: every arm runs (clamping is fine — coverage, not motion).
    for (code, mods) in [
        (KeyCode::Char('L'), KeyModifiers::NONE),
        (KeyCode::Char('H'), KeyModifiers::NONE),
        (KeyCode::Right, KeyModifiers::SHIFT),
        (KeyCode::Left, KeyModifiers::SHIFT),
        (KeyCode::Right, KeyModifiers::NONE),
        (KeyCode::Char('l'), KeyModifiers::NONE),
        (KeyCode::Left, KeyModifiers::NONE),
        (KeyCode::Char('h'), KeyModifiers::NONE),
    ] {
        handle_key(&mut app, code, mods);
    }
    assert!(!app.should_quit, "scrolling never quits");
}
