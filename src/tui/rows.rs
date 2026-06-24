//! The single row-planning layer: flatten a changeset into a flat list of rows.
//! Windowing, scrolling, and navigation all derive from this one structure.
//!
//! One parametric [`Plan`] serves both layouts: the chrome rows (file/hunk
//! headers, collapsed/pending placeholders, spacers) are identical, and only the
//! body differs — a stacked [`Row::Line`] in unified layout, or a side-by-side
//! [`Row::Pair`] in split layout. `Plan::build` branches only in the hunk-body
//! emission; the row count and order genuinely differ between layouts, so a plan
//! is built per layout (the active one).

use std::collections::BTreeSet;

use crate::model::{parent_dir, Changeset, LayoutMode, LineKind};

/// One renderable row in the review stream. The body variant present depends on
/// the plan's `layout`: `Line` in stacked layout, `Pair` in split layout.
pub enum Row {
    /// A file header; carries the index into `Changeset::files`.
    FileHeader(usize),
    /// A collapsed (reviewed) file body placeholder; carries hidden-hunk count.
    Collapsed(usize),
    /// A folded directory's body placeholder: stands in for all its (hidden)
    /// files. `reviewed` is how many of the `n` files are reviewed (so the body
    /// can show `Y/N` and a `✓` once the whole directory is done).
    CollapsedDir {
        dir: String,
        n: usize,
        reviewed: usize,
    },
    /// An undiffed file's body placeholder, shown while its diff streams in.
    Pending,
    /// A commit-message banner line, prepended above the diff for a commit view.
    /// Part of the scrollable plan, so it scrolls away as the user reads down.
    Banner(String),
    /// A hunk boundary, rendered as a dim `⋯` gap in the interactive view (the
    /// `@@ … @@` ranges live only in the lazygit-compatible renderers). Carries the
    /// previous hunk's last new-side line number — the smaller-digit surrounding
    /// number — used only to left-align the `⋯` under the gutter numbers.
    HunkHeader(u32),
    /// A stacked (unified) diff content line. `file` is the index into
    /// `Changeset::files` (for highlight-cache lookup).
    Line {
        file: usize,
        kind: LineKind,
        old: Option<u32>,
        new: Option<u32>,
        text: String,
        emphasis: Option<(u32, u32)>,
    },
    /// A split (side-by-side) row: deletions on the left paired with insertions
    /// on the right (either side may be blank).
    Pair(Option<SplitCell>, Option<SplitCell>),
    /// Blank separator between files.
    Spacer,
}

/// One side of a split (side-by-side) row, or blank.
pub struct SplitCell {
    pub file: usize,
    pub side_new: bool,
    pub lineno: Option<u32>,
    pub kind: LineKind,
    pub text: String,
    pub emphasis: Option<(u32, u32)>,
}

/// The flattened plan plus the indices needed for navigation. `layout` records
/// which layout these rows were built for.
pub struct Plan {
    pub rows: Vec<Row>,
    /// Row index where each *visible* file's header sits, parallel to
    /// `visible_files` (NOT to `Changeset::files`): `file_starts[k]` is the row of
    /// the k-th visible file. With nothing folded this is the dense per-file index.
    pub file_starts: Vec<usize>,
    /// Original `Changeset::files` index of each visible file, in stream order.
    /// Parallel to `file_starts`. Identity (`0..n`) when nothing is folded.
    pub visible_files: Vec<usize>,
    /// Row indices of every hunk header, in stream order.
    pub hunk_starts: Vec<usize>,
    /// Widest rendered row (columns), for clamping horizontal scroll. In split
    /// layout this is the widest single cell (one column).
    pub content_w: usize,
    /// The layout these rows were built for.
    pub layout: LayoutMode,
}

/// Columns the line-number gutter + sign prefix occupy before the body text.
const GUTTER_W: usize = 6;

impl Plan {
    /// Build the plan for `layout` from a changeset. Files marked viewed are
    /// collapsed to a single placeholder row (their hunks are hidden). Files whose
    /// parent directory is in `collapsed` are folded out entirely — no header, no
    /// hunks — replaced by one [`Row::CollapsedDir`] placeholder per directory. The
    /// chrome is identical across layouts; only the hunk body differs (a unified
    /// line vs a paired split row).
    pub fn build(
        cs: &Changeset,
        viewed: &[bool],
        layout: LayoutMode,
        collapsed: &BTreeSet<String>,
    ) -> Plan {
        Self::build_with_banner(cs, viewed, layout, collapsed, &[])
    }

    /// Like [`Plan::build`], but prepends `banner` (one [`Row::Banner`] per line,
    /// then a spacer) ahead of the first file — the commit-message banner. Because
    /// the navigation indices are computed from `rows.len()` as rows are pushed,
    /// the banner offset flows into `file_starts`/`hunk_starts` automatically and
    /// all navigation stays correct.
    pub fn build_with_banner(
        cs: &Changeset,
        viewed: &[bool],
        layout: LayoutMode,
        collapsed: &BTreeSet<String>,
        banner: &[String],
    ) -> Plan {
        let split = matches!(layout, LayoutMode::Split);
        let mut rows = Vec::new();
        let mut file_starts = Vec::new();
        let mut visible_files = Vec::new();
        let mut hunk_starts = Vec::new();
        prepend_banner(&mut rows, banner);
        let mut content_w = 0;

        let mut prev_dir: Option<&str> = None;
        for (fi, f) in cs.files.iter().enumerate() {
            let dir = parent_dir(&f.path);
            let first_of_dir = prev_dir != Some(dir);
            prev_dir = Some(dir);

            // A folded directory: emit one placeholder on its first file (files of
            // a directory are contiguous, since they are sorted by parent), then
            // skip every file in it.
            if collapsed.contains(dir) {
                if first_of_dir {
                    #[expect(
                        clippy::indexing_slicing,
                        reason = "fi is an enumerate index into cs.files"
                    )]
                    let n = cs.files[fi..]
                        .iter()
                        .take_while(|g| parent_dir(&g.path) == dir)
                        .count();
                    let reviewed = (fi..fi + n)
                        .filter(|&k| viewed.get(k).copied().unwrap_or(false))
                        .count();
                    rows.push(Row::CollapsedDir {
                        dir: dir.to_string(),
                        n,
                        reviewed,
                    });
                    rows.push(Row::Spacer);
                }
                continue;
            }

            file_starts.push(rows.len());
            visible_files.push(fi);
            rows.push(Row::FileHeader(fi));

            if viewed.get(fi).copied().unwrap_or(false) {
                rows.push(Row::Collapsed(f.hunks.len()));
                rows.push(Row::Spacer);
                continue;
            }

            // Not yet diffed: a single placeholder row stands in for the body
            // until the background diff lands.
            if !f.diffed {
                rows.push(Row::Pending);
                rows.push(Row::Spacer);
                continue;
            }

            if f.is_binary {
                // Split layout has no sensible two-column body for a binary file,
                // so it shows nothing; the stacked layout shows a note line.
                if split {
                    rows.push(Row::Spacer);
                    continue;
                }
                rows.push(Row::Line {
                    file: fi,
                    kind: LineKind::Context,
                    old: None,
                    new: None,
                    text: "Binary file — no preview".to_string(),
                    emphasis: None,
                });
            }

            // The gap marker before each hunk is aligned to the previous hunk's
            // last new-side line (the smaller-digit surrounding number, which
            // precedes — so is ≤ — this hunk's numbers). Tracked across iterations
            // so the first hunk (no predecessor) simply gets no marker.
            let mut prev_hunk_end: Option<u32> = None;
            for h in &f.hunks {
                hunk_starts.push(rows.len());
                if let Some(above) = prev_hunk_end {
                    rows.push(Row::HunkHeader(above));
                }
                if split {
                    for l in &h.lines {
                        content_w = content_w.max(GUTTER_W + l.text.chars().count());
                    }
                    emit_split_body(&mut rows, fi, &h.lines);
                } else {
                    for l in &h.lines {
                        content_w = content_w.max(GUTTER_W + l.text.chars().count());
                        rows.push(Row::Line {
                            file: fi,
                            kind: l.kind,
                            old: l.old_lineno,
                            new: l.new_lineno,
                            text: l.text.clone(),
                            emphasis: l.emphasis,
                        });
                    }
                }
                prev_hunk_end = Some(h.new_start + h.new_len.saturating_sub(1));
            }
            rows.push(Row::Spacer);
        }

        Plan {
            rows,
            file_starts,
            visible_files,
            hunk_starts,
            content_w,
            layout,
        }
    }

    /// The visible ordinal (index into `file_starts`/`visible_files`) of the file
    /// at `Changeset::files` index `fi`, or `None` when it is folded away.
    pub fn visible_ordinal(&self, fi: usize) -> Option<usize> {
        self.visible_files.iter().position(|&i| i == fi)
    }

    /// Row index of the folded-directory placeholder for `dir`, if present.
    pub fn collapsed_row(&self, dir: &str) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| matches!(r, Row::CollapsedDir { dir: d, .. } if d == dir))
    }
}

/// Prepend the commit-message banner (one [`Row::Banner`] per line, then a
/// spacer) to `rows`. Banner rows never pan with `h_scroll` (the renderer draws
/// them fixed), so they deliberately do not contribute to `content_w` — a long
/// message line must not widen the horizontal scroll range of the diff body.
/// Empty `banner` pushes nothing.
fn prepend_banner(rows: &mut Vec<Row>, banner: &[String]) {
    for line in banner {
        rows.push(Row::Banner(line.clone()));
    }
    if !banner.is_empty() {
        rows.push(Row::Spacer);
    }
}

/// Emit a hunk's lines as split `Pair` rows: removals are paired with insertions
/// row-for-row, context lines align on both sides.
fn emit_split_body(rows: &mut Vec<Row>, fi: usize, lines: &[crate::model::Line]) {
    let mut rem: Vec<&crate::model::Line> = Vec::new();
    let mut add: Vec<&crate::model::Line> = Vec::new();
    let flush = |rows: &mut Vec<Row>,
                 rem: &mut Vec<&crate::model::Line>,
                 add: &mut Vec<&crate::model::Line>| {
        let n = rem.len().max(add.len());
        for i in 0..n {
            let left = rem.get(i).map(|l| SplitCell {
                file: fi,
                side_new: false,
                lineno: l.old_lineno,
                kind: LineKind::Removed,
                text: l.text.clone(),
                emphasis: l.emphasis,
            });
            let right = add.get(i).map(|l| SplitCell {
                file: fi,
                side_new: true,
                lineno: l.new_lineno,
                kind: LineKind::Added,
                text: l.text.clone(),
                emphasis: l.emphasis,
            });
            rows.push(Row::Pair(left, right));
        }
        rem.clear();
        add.clear();
    };

    for l in lines {
        match l.kind {
            LineKind::Context => {
                flush(rows, &mut rem, &mut add);
                let left = SplitCell {
                    file: fi,
                    side_new: false,
                    lineno: l.old_lineno,
                    kind: LineKind::Context,
                    text: l.text.clone(),
                    emphasis: None,
                };
                let right = SplitCell {
                    file: fi,
                    side_new: true,
                    lineno: l.new_lineno,
                    kind: LineKind::Context,
                    text: l.text.clone(),
                    emphasis: None,
                };
                rows.push(Row::Pair(Some(left), Some(right)));
            }
            LineKind::Removed => rem.push(l),
            LineKind::Added => add.push(l),
        }
    }
    flush(rows, &mut rem, &mut add);
}

/// Index of the file whose region contains `row` (the last start <= row).
pub fn file_at(starts: &[usize], row: usize) -> usize {
    match starts.binary_search(&row) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiffFile, FileStatus, Hunk, Line, Stats};

    fn nofold() -> BTreeSet<String> {
        BTreeSet::new()
    }

    fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Line {
        Line {
            kind,
            old_lineno: old,
            new_lineno: new,
            text: text.into(),
            emphasis: None,
        }
    }

    /// One file with a hunk: 1 context, 2 removed, 1 added.
    fn fixture() -> Changeset {
        let hunk = Hunk {
            old_start: 1,
            old_len: 3,
            new_start: 1,
            new_len: 2,
            lines: vec![
                line(LineKind::Context, Some(1), Some(1), "ctx"),
                line(LineKind::Removed, Some(2), None, "a"),
                line(LineKind::Removed, Some(3), None, "b"),
                line(LineKind::Added, None, Some(2), "c"),
            ],
        };
        let f = DiffFile {
            path: "f.rs".into(),
            previous_path: None,
            status: FileStatus::Modified,
            staged: false,
            hunks: vec![hunk],
            stats: Stats {
                additions: 1,
                deletions: 2,
            },
            language: None,
            is_binary: false,
            old_text: None,
            new_text: None,
            diffed: true,
        };
        Changeset {
            source: "t".into(),
            files: vec![f],
        }
    }

    fn kinds(rows: &[Row]) -> Vec<&'static str> {
        rows.iter()
            .map(|r| match r {
                Row::FileHeader(_) => "fh",
                Row::Collapsed(_) => "col",
                Row::CollapsedDir { .. } => "cdir",
                Row::Pending => "pend",
                Row::Banner(_) => "ban",
                Row::HunkHeader(_) => "hh",
                Row::Line { .. } => "line",
                Row::Pair(..) => "pair",
                Row::Spacer => "sp",
            })
            .collect()
    }

    #[test]
    fn stack_sequence_interleaves_lines() {
        let cs = fixture();
        let p = Plan::build(&cs, &[false], LayoutMode::Stack, &nofold());
        // header, 4 body lines (ctx, rem, rem, add), spacer. The first (only) hunk
        // has no `⋯` gap marker — it follows the file header directly.
        assert_eq!(kinds(&p.rows), ["fh", "line", "line", "line", "line", "sp"]);
        assert_eq!(p.file_starts, vec![0]);
        assert_eq!(p.hunk_starts, vec![1]);
    }

    #[test]
    fn split_sequence_pairs_removed_with_added() {
        let cs = fixture();
        let p = Plan::build(&cs, &[false], LayoutMode::Split, &nofold());
        // header, then: ctx pair, then 2 removed paired with 1 added (max(2,1) = 2
        // pair rows), spacer. No `⋯` before the first hunk.
        assert_eq!(kinds(&p.rows), ["fh", "pair", "pair", "pair", "sp"]);
        assert_eq!(p.hunk_starts, vec![1]);
    }

    #[test]
    fn change_starts_are_consistent_per_layout() {
        let cs = fixture();
        // The first changed row in stack is the first Removed line (row index 2,
        // after fh + ctx-line; no leading hunk marker). In split it is the first
        // non-context pair.
        let stack = Plan::build(&cs, &[false], LayoutMode::Stack, &nofold());
        let first_change = stack.rows.iter().position(|r| {
            matches!(
                r,
                Row::Line {
                    kind: LineKind::Removed,
                    ..
                }
            )
        });
        assert_eq!(first_change, Some(2));
        let split = Plan::build(&cs, &[false], LayoutMode::Split, &nofold());
        let first_pair_change = split
            .rows
            .iter()
            .position(|r| matches!(r, Row::Pair(Some(c), _) if c.kind != LineKind::Context));
        assert_eq!(first_pair_change, Some(2));
    }

    #[test]
    fn first_hunk_has_no_gap_marker_but_later_hunks_do() {
        // Two hunks: the first (new lines 10–12) has no `⋯`; the second (starting
        // at new line 120) does. The digit count changes across the boundary, so
        // the marker aligns to the smaller — the first hunk's last line, 12.
        let hunk = |new_start: u32, new_len: u32| Hunk {
            old_start: new_start,
            old_len: new_len,
            new_start,
            new_len,
            lines: (0..new_len)
                .map(|i| line(LineKind::Added, None, Some(new_start + i), "x"))
                .collect(),
        };
        let f = DiffFile {
            path: "f.rs".into(),
            previous_path: None,
            status: FileStatus::Modified,
            staged: false,
            hunks: vec![hunk(10, 3), hunk(120, 1)],
            stats: Stats::default(),
            language: None,
            is_binary: false,
            old_text: None,
            new_text: None,
            diffed: true,
        };
        let cs = Changeset {
            source: "t".into(),
            files: vec![f],
        };
        let p = Plan::build(&cs, &[false], LayoutMode::Stack, &nofold());
        // fh, 3 lines (hunk 0, no marker), hh (⋯ before hunk 1), line, sp.
        assert_eq!(
            kinds(&p.rows),
            ["fh", "line", "line", "line", "hh", "line", "sp"]
        );
        // One gap marker, carrying the first hunk's last new-side line (12) — the
        // smaller-digit surrounding number — not the second hunk's start (120).
        let markers: Vec<u32> = p
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::HunkHeader(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(markers, vec![12]);
    }

    /// Two files in `src/`, one at root. Folding `src` drops both `src` files'
    /// rows, leaving one placeholder; the root file still renders in full.
    fn two_dir_fixture() -> Changeset {
        let mk = |path: &str| DiffFile {
            path: path.into(),
            previous_path: None,
            status: FileStatus::Modified,
            staged: false,
            hunks: vec![Hunk {
                old_start: 1,
                old_len: 1,
                new_start: 1,
                new_len: 1,
                lines: vec![line(LineKind::Added, None, Some(1), "x")],
            }],
            stats: Stats {
                additions: 1,
                deletions: 0,
            },
            language: None,
            is_binary: false,
            old_text: None,
            new_text: None,
            diffed: true,
        };
        // Sorted by (parent_dir, name): root file first, then the two src files.
        Changeset {
            source: "t".into(),
            files: vec![mk("a.rs"), mk("src/b.rs"), mk("src/c.rs")],
        }
    }

    #[test]
    fn folded_directory_yields_one_placeholder_and_no_file_rows() {
        let cs = two_dir_fixture();
        let mut collapsed = BTreeSet::new();
        collapsed.insert("src".to_string());
        let p = Plan::build(&cs, &[false, false, false], LayoutMode::Stack, &collapsed);

        // The two src files leave no FileHeader rows; exactly one CollapsedDir.
        let headers: Vec<usize> = p
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::FileHeader(i) => Some(*i),
                _ => None,
            })
            .collect();
        assert_eq!(
            headers,
            vec![0],
            "only the root file has a header; src is folded"
        );
        let cdirs = p
            .rows
            .iter()
            .filter(|r| matches!(r, Row::CollapsedDir { .. }))
            .count();
        assert_eq!(cdirs, 1, "one placeholder for the folded directory");
        // file_starts / visible_files re-index over the visible file only.
        assert_eq!(p.visible_files, vec![0], "only the root file is visible");
        assert_eq!(p.visible_ordinal(0), Some(0));
        assert_eq!(
            p.visible_ordinal(1),
            None,
            "folded file has no visible ordinal"
        );
        assert!(
            p.collapsed_row("src").is_some(),
            "the placeholder row is locatable"
        );
    }

    #[test]
    fn banner_rows_precede_files_and_offset_indices() {
        let cs = fixture();
        let banner = vec![
            "abc123 · me · 2026-06-30".to_string(),
            String::new(),
            "the message".to_string(),
        ];
        let p = Plan::build_with_banner(&cs, &[false], LayoutMode::Stack, &nofold(), &banner);
        // Three banner rows then a spacer precede the file header.
        assert_eq!(&kinds(&p.rows)[..4], ["ban", "ban", "ban", "sp"]);
        assert_eq!(
            p.file_starts,
            vec![4],
            "the file header is offset past the banner+spacer"
        );
        assert_eq!(
            p.hunk_starts,
            vec![5],
            "hunk index carries the banner offset"
        );
        // No banner → the plan is unchanged (file at row 0).
        let p0 = Plan::build(&cs, &[false], LayoutMode::Stack, &nofold());
        assert_eq!(p0.file_starts, vec![0]);
    }

    #[test]
    fn viewed_file_collapses_to_one_placeholder() {
        let cs = fixture();
        let p = Plan::build(&cs, &[true], LayoutMode::Stack, &nofold());
        // A reviewed file's body is hidden behind a single Collapsed placeholder.
        assert_eq!(kinds(&p.rows), ["fh", "col", "sp"]);
    }

    #[test]
    fn binary_file_body_differs_by_layout() {
        let mut cs = fixture();
        cs.files[0].is_binary = true;
        cs.files[0].hunks.clear();
        // Stacked layout shows a one-line "no preview" note.
        let stack = Plan::build(&cs, &[false], LayoutMode::Stack, &nofold());
        assert_eq!(kinds(&stack.rows), ["fh", "line", "sp"]);
        // Split layout has no two-column body for a binary, so it shows nothing.
        let split = Plan::build(&cs, &[false], LayoutMode::Split, &nofold());
        assert_eq!(kinds(&split.rows), ["fh", "sp"]);
    }

    #[test]
    fn file_at_locates_the_region_containing_a_row() {
        let starts = vec![0, 5, 12];
        assert_eq!(file_at(&starts, 0), 0, "exact match on a start");
        assert_eq!(file_at(&starts, 5), 1, "exact match on a later start");
        assert_eq!(file_at(&starts, 3), 0, "between starts → previous region");
        assert_eq!(file_at(&starts, 20), 2, "past the last start → last region");
        // Err(0): a row before the first start maps to region 0.
        let starts2 = vec![3, 8];
        assert_eq!(file_at(&starts2, 1), 0, "before the first start → 0");
    }

    #[test]
    fn collapsed_and_pending_chrome_match_across_layouts() {
        let mut cs = fixture();
        cs.files[0].diffed = false;
        for layout in [LayoutMode::Stack, LayoutMode::Split] {
            let p = Plan::build(&cs, &[false], layout, &nofold());
            assert_eq!(
                kinds(&p.rows),
                ["fh", "pend", "sp"],
                "undiffed chrome is layout-independent"
            );
        }
    }
}
