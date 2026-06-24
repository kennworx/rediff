//! Sidebar windowing, hit-testing, and digit jumps: pure geometry over the file
//! list and a view's [`ViewState`]. Free functions — no `App` — so the math is
//! unit-testable in isolation; the caller applies focus/jump side effects.

use std::collections::BTreeSet;

use ratatui::layout::Rect;

use crate::model::{parent_dir, DiffFile};
use crate::tui::view::ViewState;

/// How the sidebar file list is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    /// A plain list, one row per file.
    Flat,
    /// Files grouped under a directory line per parent directory.
    ByDir,
}

impl Grouping {
    /// The other mode (for the `g` toggle).
    pub fn toggled(self) -> Grouping {
        match self {
            Grouping::Flat => Grouping::ByDir,
            Grouping::ByDir => Grouping::Flat,
        }
    }
}

/// One rendered row of the sidebar: a directory line (chrome), a file, or a
/// folded directory's placeholder standing in for its hidden files.
pub enum SidebarRow {
    /// A directory line carrying the group's parent path (`""` renders as `./`).
    Dir(String),
    /// A file row carrying its index into `cs.files`.
    File(usize),
    /// A folded directory's placeholder: the parent path and how many files it
    /// hides. Selectable (its verb is unfold); replaces the directory's `File`
    /// rows while folded.
    CollapsedFiles { dir: String, n: usize },
}

impl SidebarRow {
    /// The file index, if this is a file row (placeholders and dir lines: `None`).
    pub fn file(&self) -> Option<usize> {
        match self {
            SidebarRow::File(i) => Some(*i),
            SidebarRow::Dir(_) | SidebarRow::CollapsedFiles { .. } => None,
        }
    }
}

/// A cursor stop in the sidebar: a visible file, or a folded directory's
/// placeholder. Directory headers are chrome and never a stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nav {
    File(usize),
    Dir(String),
}

/// What a click landed on: a file (select + jump) or a directory line /
/// placeholder (toggle its fold).
pub enum RowHit {
    File(usize),
    Dir(String),
}

/// Build the sidebar's rows for `files` under `grouping`, with `collapsed`
/// directories folded. `Flat` is one `File` per file (row index == file index;
/// collapse does not apply — there are no directory lines). `ByDir` inserts a
/// `Dir` line per parent directory, then either its `File` rows or — when the
/// directory is folded — a single `CollapsedFiles` placeholder. Relies on `files`
/// being path-sorted (a directory's files contiguous), so each appears once.
#[expect(
    clippy::indexing_slicing,
    reason = "files[i] is guarded by the enclosing `i < files.len()` loop conditions"
)]
pub fn rows(
    files: &[DiffFile],
    grouping: Grouping,
    collapsed: &BTreeSet<String>,
) -> Vec<SidebarRow> {
    match grouping {
        Grouping::Flat => (0..files.len()).map(SidebarRow::File).collect(),
        Grouping::ByDir => {
            let mut out = Vec::with_capacity(files.len() + 8);
            let mut i = 0;
            while i < files.len() {
                let dir = parent_dir(&files[i].path);
                let start = i;
                while i < files.len() && parent_dir(&files[i].path) == dir {
                    i += 1;
                }
                out.push(SidebarRow::Dir(dir.to_string()));
                if collapsed.contains(dir) {
                    out.push(SidebarRow::CollapsedFiles {
                        dir: dir.to_string(),
                        n: i - start,
                    });
                } else {
                    out.extend((start..i).map(SidebarRow::File));
                }
            }
            out
        }
    }
}

/// The navigable cursor sequence — visible files and collapsed placeholders, in
/// row order — that `j`/`k`, `{`/`}`, and Space step across (directory headers
/// skipped).
pub fn nav_sequence(rows: &[SidebarRow]) -> Vec<Nav> {
    rows.iter()
        .filter_map(|r| match r {
            SidebarRow::File(i) => Some(Nav::File(*i)),
            SidebarRow::CollapsedFiles { dir, .. } => Some(Nav::Dir(dir.clone())),
            SidebarRow::Dir(_) => None,
        })
        .collect()
}

/// The cursor's position in the nav sequence (file index, or selected placeholder).
fn nav_pos(st: &ViewState, seq: &[Nav]) -> Option<usize> {
    seq.iter().position(|n| match (n, &st.selected_dir) {
        (Nav::Dir(d), Some(sd)) => d == sd,
        (Nav::File(i), None) => *i == st.selected,
        _ => false,
    })
}

/// Step the selection by `delta` through the nav sequence, clamped, and request a
/// reveal. Returns the new cursor stop (or `None` if the sequence is empty).
#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    reason = "seq is non-empty here; positions/lengths are small row counts, and `next` is clamped to [0, seq.len()-1] so the cast back is non-negative and the index in bounds"
)]
pub fn step(st: &mut ViewState, seq: &[Nav], delta: isize) -> Option<Nav> {
    if seq.is_empty() {
        return None;
    }
    let cur = nav_pos(st, seq).unwrap_or(0) as isize;
    let next = (cur + delta).clamp(0, seq.len() as isize - 1) as usize;
    apply_nav(st, &seq[next]);
    st.reveal_selected = true;
    Some(seq[next].clone())
}

/// Point the cursor at a nav stop (a file clears any placeholder; a placeholder
/// keeps the current file as its nearby fallback).
fn apply_nav(st: &mut ViewState, n: &Nav) {
    match n {
        Nav::File(i) => st.select_file(*i),
        Nav::Dir(d) => {
            let near = st.selected;
            st.select_dir(d.clone(), near);
        }
    }
}

/// The row index of file `idx`, if it is present in `rows`.
pub fn row_of_file(rows: &[SidebarRow], idx: usize) -> Option<usize> {
    rows.iter().position(|r| r.file() == Some(idx))
}

/// The row index of the current selection — a file row, or the placeholder row of
/// the selected folded directory.
pub fn row_of_selection(rows: &[SidebarRow], st: &ViewState) -> Option<usize> {
    match &st.selected_dir {
        Some(dir) => rows
            .iter()
            .position(|r| matches!(r, SidebarRow::CollapsedFiles { dir: d, .. } if d == dir)),
        None => row_of_file(rows, st.selected),
    }
}

/// Map a click at `(x, y)` to the sidebar row it lands on, within the visible
/// window. Returns `None` for clicks outside the sidebar area or below the last
/// visible row. A directory header and a folded placeholder both resolve to
/// `RowHit::Dir` (a click there toggles the fold).
pub fn row_at(
    area: Rect,
    top: usize,
    visible: usize,
    x: u16,
    y: u16,
    rows: &[SidebarRow],
) -> Option<RowHit> {
    if x < area.x || x >= area.x + area.width || y < area.y || y >= area.y + area.height {
        return None;
    }
    let row = (y - area.y) as usize;
    if row >= visible {
        return None;
    }
    match rows.get(top + row)? {
        SidebarRow::File(i) => Some(RowHit::File(*i)),
        SidebarRow::Dir(d) => Some(RowHit::Dir(d.clone())),
        SidebarRow::CollapsedFiles { dir, .. } => Some(RowHit::Dir(dir.clone())),
    }
}

/// Map a click to a sidebar *file* index, if it falls on a file row (a directory
/// line or folded placeholder yields `None`). Thin wrapper over [`row_at`].
#[cfg(test)] // used only by tests
pub fn file_at_row(
    area: Rect,
    top: usize,
    visible: usize,
    x: u16,
    y: u16,
    rows: &[SidebarRow],
) -> Option<usize> {
    match row_at(area, top, visible, x, y, rows)? {
        RowHit::File(i) => Some(i),
        RowHit::Dir(_) => None,
    }
}

/// The file indices among the rows in the window `[top, top + visible)`, in order
/// — the units the jump digits spread across (directory lines are skipped).
fn visible_files(rows: &[SidebarRow], top: usize, visible: usize) -> Vec<usize> {
    rows.iter()
        .skip(top)
        .take(visible)
        .filter_map(SidebarRow::file)
        .collect()
}

/// The file a sparse digit (1–9) maps to across the visible *files* in the
/// window. `None` when no files are visible.
#[expect(
    clippy::indexing_slicing,
    reason = "files is non-empty here and the index is clamped to files.len()-1"
)]
pub fn digit_target(d: usize, top: usize, visible: usize, rows: &[SidebarRow]) -> Option<usize> {
    let files = visible_files(rows, top, visible);
    if files.is_empty() {
        return None;
    }
    let off = digit_to_offset(d, files.len());
    Some(files[off.min(files.len() - 1)])
}

/// Scroll the row window by `delta`, clamped, without moving the selection.
/// Returns the new window top (a row index).
#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "row counts are small; the result is clamped to [0, max_top] so the cast back to usize is non-negative"
)]
pub fn scroll(top: usize, height: usize, n_rows: usize, delta: isize) -> usize {
    let h = height.max(1);
    let max_top = n_rows.saturating_sub(h) as isize;
    (top as isize + delta).clamp(0, max_top) as usize
}

/// Recompute the window `(top, visible)` over `rows` for a viewport of `height`
/// rows. `top`/`visible` are ROW indices/counts. Reveals the selected *file*'s
/// row only when `st.reveal_selected` is set (then clears it), so a manual scroll
/// — which leaves the selection put — is preserved. In flat mode the selected
/// file's row equals its index, so this reduces to the plain list behavior.
pub fn window(
    st: &mut ViewState,
    top: usize,
    height: usize,
    rows: &[SidebarRow],
) -> (usize, usize) {
    let h = height.max(1);
    let n = rows.len();
    let mut new_top = top;
    if st.reveal_selected {
        if let Some(sel_row) = row_of_selection(rows, st) {
            if sel_row < new_top {
                new_top = sel_row;
            } else if sel_row >= new_top + h {
                new_top = sel_row + 1 - h;
            }
        }
        st.reveal_selected = false;
    }
    new_top = new_top.min(n.saturating_sub(h));
    let visible = h.min(n.saturating_sub(new_top)).max(1);
    (new_top, visible)
}

/// Map a digit (1–9) to a row offset within `vis` visible files. Small sets map
/// 1:1; larger sets spread the 9 digits evenly so 1=first and 9=last.
pub fn digit_to_offset(d: usize, vis: usize) -> usize {
    if vis <= 1 {
        0
    } else if vis <= 9 {
        (d - 1).min(vis - 1)
    } else {
        ((d - 1) * (vis - 1) + 4) / 8
    }
}

/// The digit whose mapping lands on `off` (for rendering sidebar badges).
pub fn offset_to_digit(off: usize, vis: usize) -> Option<usize> {
    (1..=9).find(|&d| digit_to_offset(d, vis) == off)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileStatus;

    fn mkfiles(paths: &[&str]) -> Vec<DiffFile> {
        paths
            .iter()
            .map(|p| DiffFile::stub((*p).to_string(), None, FileStatus::Modified, false, None))
            .collect()
    }

    fn nofold() -> BTreeSet<String> {
        BTreeSet::new()
    }

    fn folded(dirs: &[&str]) -> BTreeSet<String> {
        dirs.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn rows_flat_is_one_per_file() {
        let files = mkfiles(&["a.rs", "src/b.rs"]);
        let r = rows(&files, Grouping::Flat, &nofold());
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].file(), Some(0));
        assert_eq!(r[1].file(), Some(1));
    }

    #[test]
    fn rows_bydir_one_dir_line_per_directory() {
        // Path-sorted input (as enumerate produces).
        let files = mkfiles(&[
            "README.md",
            "lib/b.rs",
            "src/a.rs",
            "src/c.rs",
            "src/tui/app.rs",
        ]);
        let shape: Vec<String> = rows(&files, Grouping::ByDir, &nofold())
            .iter()
            .map(|row| match row {
                SidebarRow::Dir(d) if d.is_empty() => "[.]".to_string(),
                SidebarRow::Dir(d) => format!("[{d}]"),
                SidebarRow::File(i) => files[*i].path.clone(),
                SidebarRow::CollapsedFiles { dir, n } => format!("<{dir}:{n}>"),
            })
            .collect();
        assert_eq!(
            shape,
            vec![
                "[.]",
                "README.md",
                "[lib]",
                "lib/b.rs",
                "[src]",
                "src/a.rs",
                "src/c.rs",
                "[src/tui]",
                "src/tui/app.rs",
            ]
        );
    }

    #[test]
    fn rows_bydir_single_directory_one_header() {
        let files = mkfiles(&["src/a.rs", "src/b.rs"]);
        let dirs = rows(&files, Grouping::ByDir, &nofold())
            .iter()
            .filter(|row| matches!(row, SidebarRow::Dir(_)))
            .count();
        assert_eq!(dirs, 1, "one header for one directory");
        assert_eq!(
            row_of_file(&rows(&files, Grouping::ByDir, &nofold()), 1),
            Some(2),
            "file 1 sits after the dir line"
        );
    }

    #[test]
    fn file_at_row_honors_the_scroll_offset() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 6,
        };
        let flat: Vec<SidebarRow> = (0..12).map(SidebarRow::File).collect();
        // Window scrolled so screen rows 0..5 show files 4..9.
        assert_eq!(
            file_at_row(area, 4, 5, 1, 2, &flat),
            Some(6),
            "row 2 → file 4+2"
        );
        assert_eq!(
            file_at_row(area, 4, 5, 1, 5, &flat),
            None,
            "past the last visible row"
        );
        assert_eq!(
            file_at_row(area, 4, 5, 40, 2, &flat),
            None,
            "outside the sidebar columns"
        );
    }

    #[test]
    fn click_and_digits_skip_directory_lines() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 10,
        };
        // rows: Dir, File0, File1, Dir, File2
        let r = vec![
            SidebarRow::Dir("src".into()),
            SidebarRow::File(0),
            SidebarRow::File(1),
            SidebarRow::Dir("lib".into()),
            SidebarRow::File(2),
        ];
        // Click on the directory line (row 0) selects nothing; on a file row it does.
        assert_eq!(
            file_at_row(area, 0, 5, 1, 0, &r),
            None,
            "click on a Dir line → no file"
        );
        assert_eq!(
            file_at_row(area, 0, 5, 1, 1, &r),
            Some(0),
            "click on the first file row"
        );
        assert_eq!(
            file_at_row(area, 0, 5, 1, 4, &r),
            Some(2),
            "click on a file under the 2nd dir"
        );
        // Digits spread over the 3 visible files only (dirs skipped).
        assert_eq!(digit_target(1, 0, 5, &r), Some(0), "1 → first visible file");
        assert_eq!(digit_target(9, 0, 5, &r), Some(2), "9 → last visible file");
    }

    #[test]
    fn window_reveals_the_selected_files_row_under_a_dir_line() {
        // rows: Dir, File0, File1, Dir, File2(@row4), File3
        let r = vec![
            SidebarRow::Dir("a".into()),
            SidebarRow::File(0),
            SidebarRow::File(1),
            SidebarRow::Dir("b".into()),
            SidebarRow::File(2),
            SidebarRow::File(3),
        ];
        let mut st = ViewState {
            selected: 2,
            reveal_selected: true,
            ..Default::default()
        };
        let (top, visible) = window(&mut st, 0, 3, &r);
        assert!(
            top <= 4 && 4 < top + visible,
            "selected file's row (4) is in the window"
        );
        assert!(!st.reveal_selected, "reveal consumed");
    }

    #[test]
    fn step_walks_files_and_clamps() {
        let r: Vec<SidebarRow> = (0..5).map(SidebarRow::File).collect();
        let seq = nav_sequence(&r);
        let mut st = ViewState {
            selected: 0,
            ..Default::default()
        };
        assert_eq!(
            step(&mut st, &seq, -1),
            Some(Nav::File(0)),
            "clamped at the top"
        );
        assert!(st.reveal_selected);
        assert_eq!(
            step(&mut st, &seq, 99),
            Some(Nav::File(4)),
            "clamped at the last file"
        );
        assert_eq!(step(&mut st, &[], 1), None, "empty sequence → no move");
    }

    #[test]
    fn folded_directory_yields_header_plus_one_placeholder() {
        // src has two files; folding it replaces both with one placeholder.
        let files = mkfiles(&["a.rs", "src/b.rs", "src/c.rs"]);
        let r = rows(&files, Grouping::ByDir, &folded(&["src"]));
        let shape: Vec<String> = r
            .iter()
            .map(|row| match row {
                SidebarRow::Dir(d) if d.is_empty() => "[.]".to_string(),
                SidebarRow::Dir(d) => format!("[{d}]"),
                SidebarRow::File(i) => files[*i].path.clone(),
                SidebarRow::CollapsedFiles { dir, n } => format!("<{dir}:{n}>"),
            })
            .collect();
        assert_eq!(shape, vec!["[.]", "a.rs", "[src]", "<src:2>"]);
    }

    #[test]
    fn step_lands_on_a_placeholder_then_the_next_file() {
        // rows: [.] a.rs  [src] <src:2>  [z] z.rs   → nav = File(0), Dir(src), File(3)
        let files = mkfiles(&["a.rs", "src/b.rs", "src/c.rs", "z/z.rs"]);
        let r = rows(&files, Grouping::ByDir, &folded(&["src"]));
        let seq = nav_sequence(&r);
        let mut st = ViewState {
            selected: 0,
            ..Default::default()
        };
        // a.rs → the src placeholder.
        assert_eq!(step(&mut st, &seq, 1), Some(Nav::Dir("src".into())));
        assert_eq!(
            st.selected_dir.as_deref(),
            Some("src"),
            "cursor is on the placeholder"
        );
        assert_eq!(
            st.selected_file(),
            None,
            "file actions inert on a placeholder"
        );
        // placeholder → the file after the fold.
        assert_eq!(step(&mut st, &seq, 1), Some(Nav::File(3)));
        assert_eq!(st.selected_file(), Some(3));
        assert!(
            st.selected_dir.is_none(),
            "moving onto a file clears the placeholder"
        );
    }

    #[test]
    fn click_on_placeholder_or_header_targets_the_directory() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 10,
        };
        // rows: [.] File0  [src] <src:2>
        let r = vec![
            SidebarRow::Dir(String::new()),
            SidebarRow::File(0),
            SidebarRow::Dir("src".into()),
            SidebarRow::CollapsedFiles {
                dir: "src".into(),
                n: 2,
            },
        ];
        assert!(
            matches!(row_at(area, 0, 4, 1, 2, &r), Some(RowHit::Dir(d)) if d == "src"),
            "click the src header"
        );
        assert!(
            matches!(row_at(area, 0, 4, 1, 3, &r), Some(RowHit::Dir(d)) if d == "src"),
            "click the placeholder"
        );
        assert!(
            matches!(row_at(area, 0, 4, 1, 1, &r), Some(RowHit::File(0))),
            "click a file row"
        );
    }

    #[test]
    fn window_reveals_a_selected_placeholder_scrolled_off() {
        // A long list with the src placeholder near the end; selecting it and
        // revealing scrolls the window down to it.
        let mut r: Vec<SidebarRow> = (0..10).map(SidebarRow::File).collect();
        r.push(SidebarRow::Dir("src".into()));
        r.push(SidebarRow::CollapsedFiles {
            dir: "src".into(),
            n: 3,
        });
        let mut st = ViewState {
            selected: 0,
            ..Default::default()
        };
        st.select_dir("src".into(), 0);
        st.reveal_selected = true;
        let (top, visible) = window(&mut st, 0, 4, &r);
        let ph_row = 11; // the placeholder's row index
        assert!(
            top <= ph_row && ph_row < top + visible,
            "the placeholder row is revealed"
        );
    }

    #[test]
    fn digit_offsets_span_first_to_last() {
        assert_eq!(digit_to_offset(1, 17), 0);
        assert_eq!(digit_to_offset(9, 17), 16);
        assert_eq!(offset_to_digit(0, 17), Some(1));
        assert_eq!(offset_to_digit(16, 17), Some(9));
    }

    #[test]
    fn digit_target_is_none_without_visible_files() {
        // A window that holds only a directory line → no files to spread over.
        let r = vec![SidebarRow::Dir("src".into())];
        assert_eq!(digit_target(1, 0, 5, &r), None, "no visible files → None");
        // An empty row set is likewise None.
        assert_eq!(digit_target(9, 0, 5, &[]), None, "empty rows → None");
    }

    #[test]
    fn digit_target_clamps_past_the_last_file() {
        // Two visible files; a high digit clamps onto the last one.
        let r: Vec<SidebarRow> = (0..2).map(SidebarRow::File).collect();
        assert_eq!(digit_target(1, 0, 5, &r), Some(0), "1 → first file");
        assert_eq!(digit_target(9, 0, 5, &r), Some(1), "9 clamps to the last");
    }
}
