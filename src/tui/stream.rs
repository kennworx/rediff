//! Diff-stream navigation: pure viewport/cursor operations over a view's
//! [`ViewState`] + [`Plan`]. These are free functions — no `App` — so the main
//! stream and the single-file peek can share one navigation model. They touch
//! only the scroll/selection state, the immutable plan, and the viewport
//! geometry; never the loader, highlighter, or view stack.

use crate::model::LayoutMode;
use crate::tui::rows::{self, Plan};
use crate::tui::view::ViewState;

/// One split column's text width (mirrors `draw_split`'s column geometry).
fn split_col_w(viewport_w: usize) -> usize {
    viewport_w.saturating_sub(1) / 2
}

/// Max horizontal scroll: content width beyond the viewport. In split layout the
/// bound is one column's width, since each side pans within its own column.
pub fn max_h_scroll(plan: &Plan, viewport_w: usize) -> usize {
    let visible = if matches!(plan.layout, LayoutMode::Split) {
        split_col_w(viewport_w)
    } else {
        viewport_w
    };
    plan.content_w.saturating_sub(visible)
}

/// Max vertical scroll for a `rows`-row body in a `viewport_h`-tall viewport:
/// the last top that still fills the viewport, so the final page stays full.
pub fn max_scroll_rows(rows: usize, viewport_h: usize) -> usize {
    rows.saturating_sub(viewport_h.max(1))
}

/// Max vertical scroll: the last viewport-top that still shows content.
pub fn max_scroll(plan: &Plan, viewport_h: usize) -> usize {
    max_scroll_rows(plan.rows.len(), viewport_h)
}

/// `Changeset::files` index of the file currently at the top of the viewport.
/// `file_at` returns a *visible ordinal*; map it back through `visible_files`
/// (identity when nothing is folded). Falls back to 0 when no file is visible.
pub fn current_file(st: &ViewState, plan: &Plan) -> usize {
    let ord = rows::file_at(&plan.file_starts, st.scroll);
    plan.visible_files.get(ord).copied().unwrap_or(0)
}

/// While scrolling, the active (sidebar-highlighted) file follows the file at
/// the top of the viewport, and the sidebar reveals it.
pub fn anchor_selected(st: &mut ViewState, plan: &Plan) {
    // Scrolling always re-anchors onto a visible file (clearing any placeholder
    // selection); placeholders are reached only by explicit step/click/fold.
    st.select_file(current_file(st, plan));
    st.reveal_selected = true;
}

/// Clamp scroll and horizontal scroll into range (after a resize / plan rebuild).
pub fn clamp(st: &mut ViewState, plan: &Plan, viewport_h: usize, viewport_w: usize) {
    st.scroll = st.scroll.min(max_scroll(plan, viewport_h));
    st.h_scroll = st.h_scroll.min(max_h_scroll(plan, viewport_w));
}

/// Re-anchor a viewport `scroll` after a plan rebuild. Normally the offset within
/// the anchor file is preserved (`new_start + (scroll − old_start)`). A scroll
/// parked above the anchor file's header — the fixed commit-message banner region
/// at the top of the plan (`scroll < old_start`) — is kept exactly where it is, so
/// a streaming rebuild doesn't yank the banner off-screen.
pub fn reanchored(scroll: usize, old_start: usize, new_start: usize) -> usize {
    if scroll < old_start {
        scroll
    } else {
        new_start + (scroll - old_start)
    }
}

pub fn scroll_to(st: &mut ViewState, plan: &Plan, viewport_h: usize, row: usize) {
    st.scroll = row.min(max_scroll(plan, viewport_h));
}

#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "scroll rows/viewport heights are bounded by plan size, far below isize::MAX; clamped to >= 0 before the cast back to usize"
)]
pub fn scroll_by(st: &mut ViewState, plan: &Plan, viewport_h: usize, delta: isize) {
    let next = st.scroll as isize + delta;
    st.scroll = next.clamp(0, max_scroll(plan, viewport_h) as isize) as usize;
    anchor_selected(st, plan);
}

#[expect(
    clippy::cast_possible_wrap,
    reason = "viewport height is a small terminal dimension, far below isize::MAX"
)]
pub fn page(st: &mut ViewState, plan: &Plan, viewport_h: usize, dir: isize) {
    let step = viewport_h.saturating_sub(1).max(1) as isize;
    scroll_by(st, plan, viewport_h, dir * step);
}

#[expect(
    clippy::cast_possible_wrap,
    reason = "viewport height is a small terminal dimension, far below isize::MAX"
)]
pub fn half_page(st: &mut ViewState, plan: &Plan, viewport_h: usize, dir: isize) {
    let step = (viewport_h / 2).max(1) as isize;
    scroll_by(st, plan, viewport_h, dir * step);
}

pub fn top(st: &mut ViewState, plan: &Plan) {
    st.scroll = 0;
    anchor_selected(st, plan);
}

pub fn bottom(st: &mut ViewState, plan: &Plan, viewport_h: usize) {
    st.scroll = max_scroll(plan, viewport_h);
    anchor_selected(st, plan);
}

pub fn next_hunk(st: &mut ViewState, plan: &Plan, viewport_h: usize) {
    if let Some(row) = plan.hunk_starts.iter().copied().find(|&r| r > st.scroll) {
        scroll_to(st, plan, viewport_h, row);
        anchor_selected(st, plan);
    }
}

pub fn prev_hunk(st: &mut ViewState, plan: &Plan, viewport_h: usize) {
    if let Some(row) = plan
        .hunk_starts
        .iter()
        .copied()
        .rev()
        .find(|&r| r < st.scroll)
    {
        scroll_to(st, plan, viewport_h, row);
        anchor_selected(st, plan);
    }
}

/// Jump the viewport to a file's header row (no selection/focus change — that is
/// the coordinator's job). `idx` is a `Changeset::files` index; it is mapped to
/// its visible ordinal first. A folded file has no row, so the jump is a no-op
/// (the coordinator unfolds before jumping when landing on it by path).
pub fn jump_to_file(st: &mut ViewState, plan: &Plan, viewport_h: usize, idx: usize) {
    if let Some(ord) = plan.visible_ordinal(idx) {
        if let Some(row) = plan.file_starts.get(ord).copied() {
            scroll_to(st, plan, viewport_h, row);
        }
    }
}

/// Scroll the viewport to a folded directory's placeholder row.
pub fn jump_to_collapsed(st: &mut ViewState, plan: &Plan, viewport_h: usize, dir: &str) {
    if let Some(row) = plan.collapsed_row(dir) {
        scroll_to(st, plan, viewport_h, row);
    }
}

#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "h-scroll columns/viewport widths are bounded by plan size, far below isize::MAX; clamped to >= 0 before the cast back to usize"
)]
pub fn h_scroll_by(st: &mut ViewState, plan: &Plan, viewport_w: usize, delta: isize) {
    let max = max_h_scroll(plan, viewport_w) as isize;
    st.h_scroll = (st.h_scroll as isize + delta).clamp(0, max) as usize;
}

pub fn toggle_wrap(st: &mut ViewState) {
    st.wrap = !st.wrap;
    st.h_scroll = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::rows::Row;

    fn plan_with(rows: usize, file_starts: Vec<usize>) -> Plan {
        let visible_files = (0..file_starts.len()).collect();
        Plan {
            rows: (0..rows).map(|_| Row::Spacer).collect(),
            file_starts,
            visible_files,
            hunk_starts: Vec::new(),
            content_w: 0,
            layout: LayoutMode::Stack,
        }
    }

    #[test]
    fn reanchored_keeps_banner_region_and_preserves_in_file_offset() {
        // Banner region (scroll < old_start): the position is kept verbatim, even
        // when the anchor file's header moves.
        assert_eq!(reanchored(2, 5, 5), 2);
        assert_eq!(reanchored(0, 5, 9), 0);
        assert_eq!(reanchored(4, 5, 9), 4, "still above the header → unchanged");
        // At/after the header: the in-file offset rides to the new header row.
        assert_eq!(reanchored(5, 5, 9), 9, "header → new header");
        assert_eq!(
            reanchored(7, 5, 9),
            11,
            "offset 2 past the header preserved"
        );
    }

    #[test]
    fn scroll_by_clamps_to_the_plan() {
        let plan = plan_with(20, vec![0]);
        let mut st = ViewState::default();
        scroll_by(&mut st, &plan, 5, 1000); // far past the end
        assert_eq!(st.scroll, 15, "clamped to max_scroll = rows - viewport");
        assert_eq!(max_scroll(&plan, 5), 15);
        scroll_by(&mut st, &plan, 5, -1000);
        assert_eq!(st.scroll, 0, "clamped at the top");
    }

    #[test]
    fn jump_to_file_lands_on_the_file_start() {
        let plan = plan_with(20, vec![0, 8, 14]);
        let mut st = ViewState::default();
        jump_to_file(&mut st, &plan, 5, 1);
        assert_eq!(st.scroll, 8, "viewport top sits on file 1's header row");
        jump_to_file(&mut st, &plan, 5, 2);
        assert_eq!(st.scroll, 14);
    }

    #[test]
    fn jump_to_file_is_a_noop_for_an_unknown_file() {
        let plan = plan_with(20, vec![0, 8]);
        let mut st = ViewState {
            scroll: 5,
            ..ViewState::default()
        };
        // A file index that maps to no visible ordinal (folded/out of range)
        // leaves the viewport untouched.
        jump_to_file(&mut st, &plan, 5, 99);
        assert_eq!(st.scroll, 5, "unknown file index leaves scroll put");
    }

    #[test]
    fn prev_hunk_steps_back_and_clamps_at_the_top() {
        let mut plan = plan_with(40, vec![0]);
        plan.hunk_starts = vec![5, 15, 25];
        let mut st = ViewState {
            scroll: 30,
            ..ViewState::default()
        };
        prev_hunk(&mut st, &plan, 10);
        assert_eq!(st.scroll, 25, "lands on the nearest earlier hunk start");
        prev_hunk(&mut st, &plan, 10);
        assert_eq!(st.scroll, 15, "steps back another hunk");
        // Before the first hunk there is no earlier one → no movement.
        st.scroll = 3;
        prev_hunk(&mut st, &plan, 10);
        assert_eq!(st.scroll, 3, "no earlier hunk → unchanged");
    }
}
