//! Turn a pair of text blobs into structured hunks with context, via imara-diff.
//! Produces git-faithful hunk bodies and `@@` header ranges.

use imara_diff::{Algorithm, Diff, InternedInput};

use crate::model::{Hunk, Line, LineKind};

const DEFAULT_CONTEXT: u32 = 3;

/// Strip a single trailing line terminator for storage/display.
fn strip_eol(s: &str) -> String {
    s.strip_suffix('\n')
        .unwrap_or(s)
        .strip_suffix('\r')
        .unwrap_or(s.strip_suffix('\n').unwrap_or(s))
        .to_string()
}

/// Compute structured hunks between `old` and `new`.
/// Returns the hunks plus total (additions, deletions) line counts.
pub fn compute_hunks(old: &str, new: &str) -> (Vec<Hunk>, usize, usize) {
    compute_hunks_with_context(old, new, DEFAULT_CONTEXT)
}

/// One synthetic all-context hunk covering the whole file, for the peek's
/// content mode (full file, highlighted, no add/remove markers).
#[expect(
    clippy::cast_possible_truncation,
    reason = "line indices/counts cast to u32; a file with >u32::MAX lines is not representable here"
)]
pub fn whole_file_hunks(content: &str) -> Vec<Hunk> {
    let mut lines = Vec::new();
    for (i, line) in content.lines().enumerate() {
        lines.push(Line::context(line.to_string(), i as u32, i as u32));
    }
    if lines.is_empty() {
        return Vec::new();
    }
    let n = lines.len() as u32;
    vec![Hunk {
        old_start: 1,
        old_len: n,
        new_start: 1,
        new_len: n,
        lines,
    }]
}

#[expect(
    clippy::indexing_slicing,
    reason = "raw is indexed only within bounds enforced by the `group_end + 1 < raw.len()` and `i <= group_end < raw.len()` loop invariants; old_lines/new_lines are indexed by o/n which are clamped to old_len/new_len via .min()"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "line counts cast to u32; a file with >u32::MAX lines is not representable here"
)]
pub fn compute_hunks_with_context(old: &str, new: &str, context: u32) -> (Vec<Hunk>, usize, usize) {
    let input = InternedInput::new(old, new);
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let old_lines: Vec<&str> = input.before.iter().map(|t| input.interner[*t]).collect();
    let new_lines: Vec<&str> = input.after.iter().map(|t| input.interner[*t]).collect();
    let old_len = old_lines.len() as u32;
    let new_len = new_lines.len() as u32;

    let additions = diff.count_additions() as usize;
    let deletions = diff.count_removals() as usize;

    let raw: Vec<imara_diff::Hunk> = diff.hunks().collect();
    let mut hunks = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        // Merge change regions whose unchanged gap is within 2*context, like git.
        let mut group_end = i;
        while group_end + 1 < raw.len() {
            let cur_end = raw[group_end].before.end;
            let next_start = raw[group_end + 1].before.start;
            if next_start <= cur_end + 2 * context {
                group_end += 1;
            } else {
                break;
            }
        }

        let first = &raw[i];
        let last = &raw[group_end];
        let o_start = first.before.start.saturating_sub(context);
        let o_end = (last.before.end + context).min(old_len);
        let n_start = first.after.start.saturating_sub(context);
        let n_end = (last.after.end + context).min(new_len);

        let mut lines = Vec::new();
        let mut o = o_start;
        let mut n = n_start;
        for h in &raw[i..=group_end] {
            while o < h.before.start {
                lines.push(Line::context(strip_eol(old_lines[o as usize]), o, n));
                o += 1;
                n += 1;
            }
            while o < h.before.end {
                lines.push(Line::removed(strip_eol(old_lines[o as usize]), o));
                o += 1;
            }
            while n < h.after.end {
                lines.push(Line::added(strip_eol(new_lines[n as usize]), n));
                n += 1;
            }
        }
        while o < o_end {
            lines.push(Line::context(strip_eol(old_lines[o as usize]), o, n));
            o += 1;
            n += 1;
        }

        emphasize_intra_line(&mut lines);

        let old_count = o_end - o_start;
        let new_count = n_end - n_start;
        hunks.push(Hunk {
            old_start: if old_count == 0 { o_start } else { o_start + 1 },
            old_len: old_count,
            new_start: if new_count == 0 { n_start } else { n_start + 1 },
            new_len: new_count,
            lines,
        });

        i = group_end + 1;
    }

    (hunks, additions, deletions)
}

/// Set intra-line emphasis ranges by pairing each removed line with the
/// corresponding added line in the same change and diffing them by common
/// prefix/suffix.
#[expect(
    clippy::indexing_slicing,
    reason = "i indexes `lines` only under `i < lines.len()` guards; rem_start+k and add_start+k stay below their respective run ends since k < pairs = min(add_start-rem_start, i-add_start)"
)]
fn emphasize_intra_line(lines: &mut [Line]) {
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind != LineKind::Removed {
            i += 1;
            continue;
        }
        let rem_start = i;
        while i < lines.len() && lines[i].kind == LineKind::Removed {
            i += 1;
        }
        let add_start = i;
        while i < lines.len() && lines[i].kind == LineKind::Added {
            i += 1;
        }
        let pairs = (add_start - rem_start).min(i - add_start);
        for k in 0..pairs {
            let (o, n) = intra_ranges(&lines[rem_start + k].text, &lines[add_start + k].text);
            lines[rem_start + k].emphasis = o;
            lines[add_start + k].emphasis = n;
        }
    }
}

/// A char range `[start, end)` within a line, or `None`.
type CharRange = Option<(u32, u32)>;

/// Char ranges that differ between `old` and `new`, trimming common prefix and
/// suffix. Returns `None` for a side when the whole line changed (nothing common).
#[expect(
    clippy::indexing_slicing,
    reason = "p is bounded by `p < o.len() && p < n.len()`; the suffix index o.len()-1-s / n.len()-1-s stays in bounds because s < o.len()-p and s < n.len()-p"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "char positions within a single line cast to u32; line length far below u32::MAX"
)]
fn intra_ranges(old: &str, new: &str) -> (CharRange, CharRange) {
    let o: Vec<char> = old.chars().collect();
    let n: Vec<char> = new.chars().collect();
    let mut p = 0;
    while p < o.len() && p < n.len() && o[p] == n[p] {
        p += 1;
    }
    let mut s = 0;
    while s < o.len() - p && s < n.len() - p && o[o.len() - 1 - s] == n[n.len() - 1 - s] {
        s += 1;
    }
    // Only emphasize when there is shared context — a full rewrite gains nothing.
    if p == 0 && s == 0 {
        return (None, None);
    }
    let o_range = (p < o.len() - s).then(|| (p as u32, (o.len() - s) as u32));
    let n_range = (p < n.len() - s).then(|| (p as u32, (n.len() - s) as u32));
    (o_range, n_range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LineKind;

    #[test]
    fn intra_line_emphasis_marks_changed_word() {
        let (hunks, _, _) = compute_hunks("let x = foo();\n", "let x = bar();\n");
        let h = &hunks[0];
        let removed = h
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Removed)
            .unwrap();
        let added = h.lines.iter().find(|l| l.kind == LineKind::Added).unwrap();
        // "foo" / "bar" occupy chars 8..11 on both sides
        assert_eq!(removed.emphasis, Some((8, 11)));
        assert_eq!(added.emphasis, Some((8, 11)));
    }

    #[test]
    fn single_line_change() {
        let (hunks, adds, dels) = compute_hunks("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!((adds, dels), (1, 1));
        assert_eq!(hunks.len(), 1);
        let h = &hunks[0];
        assert_eq!(h.header(), "@@ -1,3 +1,3 @@");
        let kinds: Vec<_> = h.lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LineKind::Context,
                LineKind::Removed,
                LineKind::Added,
                LineKind::Context
            ]
        );
    }

    #[test]
    fn new_file_header_uses_zero_old_start() {
        let (hunks, adds, dels) = compute_hunks("", "x\ny\n");
        assert_eq!((adds, dels), (2, 0));
        assert_eq!(hunks[0].header(), "@@ -0,0 +1,2 @@");
    }

    #[test]
    fn distant_changes_split_into_two_hunks() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
        let new = "X\n2\n3\n4\n5\n6\n7\n8\n9\nY\n";
        let (hunks, _, _) = compute_hunks(old, new);
        assert_eq!(
            hunks.len(),
            2,
            "changes 9 lines apart should not merge with context 3"
        );
    }

    #[test]
    fn identical_input_has_no_hunks() {
        let (hunks, adds, dels) = compute_hunks("same\n", "same\n");
        assert!(hunks.is_empty());
        assert_eq!((adds, dels), (0, 0));
    }
}
