//! Reviewed-tracking operations over a view's [`ViewState`]. Free functions — no
//! `App` — so they are unit-testable in isolation; the caller rebuilds the plan
//! (collapsed rows depend on `viewed`) and moves the cursor.

use crate::tui::view::ViewState;

/// Toggle the reviewed flag for file `idx` (no-op if out of range).
pub fn toggle(st: &mut ViewState, idx: usize) {
    if let Some(v) = st.viewed.get_mut(idx) {
        *v = !*v;
    }
}

/// The next file after `start` (wrapping) that is not reviewed and not hidden (in
/// a folded directory: `hidden[i]` true → file `i` is out of scope; a short/empty
/// `hidden` slice treats the missing entries as visible). `None` if every visible
/// file is reviewed (or there are none). `n` is the file count, which equals
/// `viewed.len()` for the view's lifetime.
#[expect(
    clippy::indexing_slicing,
    reason = "idx = _ % n and n == st.viewed.len() by contract"
)]
pub fn next_unviewed_visible(
    st: &ViewState,
    start: usize,
    n: usize,
    hidden: &[bool],
) -> Option<usize> {
    if n == 0 {
        return None;
    }
    (1..=n)
        .map(|step| (start + step) % n)
        .find(|&idx| !st.viewed[idx] && !hidden.get(idx).copied().unwrap_or(false))
}

/// Count of reviewed files in the view.
pub fn count(st: &ViewState) -> usize {
    st.viewed.iter().filter(|v| **v).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(viewed: &[bool]) -> ViewState {
        ViewState {
            viewed: viewed.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn next_unviewed_wraps_and_skips_reviewed() {
        let s = st(&[true, false, true, false]);
        assert_eq!(
            next_unviewed_visible(&s, 0, 4, &[]),
            Some(1),
            "next unreviewed after 0"
        );
        assert_eq!(
            next_unviewed_visible(&s, 1, 4, &[]),
            Some(3),
            "skips the reviewed file 2"
        );
        assert_eq!(
            next_unviewed_visible(&s, 3, 4, &[]),
            Some(1),
            "wraps past the end"
        );
    }

    #[test]
    fn next_unviewed_none_when_all_reviewed_or_empty() {
        assert_eq!(next_unviewed_visible(&st(&[true, true]), 0, 2, &[]), None);
        assert_eq!(next_unviewed_visible(&st(&[]), 0, 0, &[]), None);
    }

    #[test]
    fn next_unviewed_skips_hidden_files() {
        // File 1 is unreviewed but folded away → skipped; file 3 is the next stop.
        let s = st(&[true, false, true, false]);
        let hidden = [false, true, false, false];
        assert_eq!(
            next_unviewed_visible(&s, 0, 4, &hidden),
            Some(3),
            "the folded unreviewed file is skipped"
        );
        // With the only unreviewed-visible file also hidden, nothing remains.
        let s2 = st(&[true, false, true, true]);
        let hidden2 = [false, true, false, false];
        assert_eq!(
            next_unviewed_visible(&s2, 0, 4, &hidden2),
            None,
            "all unreviewed files are folded away"
        );
    }

    #[test]
    fn toggle_and_count() {
        let mut s = st(&[false, false, false]);
        toggle(&mut s, 1);
        assert_eq!(s.viewed, vec![false, true, false]);
        assert_eq!(count(&s), 1);
        toggle(&mut s, 1);
        assert_eq!(count(&s), 0);
    }
}
