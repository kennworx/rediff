//! A tiny case-insensitive subsequence fuzzy matcher for the file palette.

/// Score `text` against `query`. `None` if `query` is not a subsequence of
/// `text`; higher scores are better matches (contiguous + early runs win).
pub fn score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let mut qi = 0;
    let mut s = 0i32;
    let mut last: Option<usize> = None;
    #[expect(
        clippy::indexing_slicing,
        reason = "qi < q.len() is checked in the same condition"
    )]
    for (ti, &c) in t.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            if let Some(lm) = last {
                if ti == lm + 1 {
                    s += 6; // contiguous run bonus
                }
            }
            if ti == 0 {
                s += 4; // start-of-string bonus
            }
            s += 1;
            last = Some(ti);
            qi += 1;
        }
    }
    if qi == q.len() {
        // Mild preference for shorter paths.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            reason = "path char counts are far below i32::MAX"
        )]
        let penalty = (t.len() as i32) / 12;
        Some(s - penalty)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn subsequence_matches_and_ranks() {
        assert!(score("auth", "src/auth.rs").is_some());
        assert!(score("xyz", "src/auth.rs").is_none());
        // contiguous match scores higher than scattered
        let contiguous = score("auth", "auth.rs").unwrap();
        let scattered = score("auth", "a_u_t_h.rs").unwrap();
        assert!(contiguous > scattered);
    }

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(score("", "anything"), Some(0));
    }
}
