//! Pure presentation helpers for the blame gutter: the compact relative-age
//! token and the fixed-width `name + age` gutter layout. Kept free of rendering
//! so they unit-test in isolation. (The per-commit color key is cached on
//! [`crate::model::BlameCommit`] when the commit is resolved; run collapsing —
//! which lines start a new commit run — is a per-visible-row comparison done at
//! render time in `draw_blame_body`.)

/// Total width of the attribution gutter, in columns (before the ` │ ` rule).
pub const GUTTER_W: usize = 12;

const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;
/// Average Gregorian month/year in seconds, so `YEAR == 12 * MONTH` exactly and
/// 12 months rolls cleanly to `1.0y`.
const MONTH: i64 = 2_629_746;
const YEAR: i64 = 31_556_952;

/// Compact relative age of `then_secs` as of `now_secs`, using the ladder
/// hours → days → months → years. Hours and days are always integers; months and
/// years carry one decimal only while the integer part is a single digit (1–9)
/// and drop it at 10+. Future timestamps clamp to `0h`.
pub fn relative_age(now_secs: i64, then_secs: i64) -> String {
    // saturating_sub guards against a corrupt (near-i64::MIN) commit timestamp
    // overflowing the subtraction (decimal_unit saturates its ×10 for the same
    // reason); future timestamps clamp to 0 ("0h").
    let d = now_secs.saturating_sub(then_secs).max(0);
    if d < DAY {
        format!("{}h", d / HOUR)
    } else if d < MONTH {
        format!("{}d", d / DAY)
    } else if d < YEAR {
        decimal_unit(d, MONTH, 'm')
    } else {
        decimal_unit(d, YEAR, 'y')
    }
}

/// Format `d / unit` with one decimal while the integer part is single-digit, as
/// an integer once it reaches 10. Integer math throughout (no float casts): the
/// fraction is truncated, never rounded, so it never over-claims the age.
fn decimal_unit(d: i64, unit: i64, suffix: char) -> String {
    // floor(value * 10); saturating so a corrupt timestamp (d = i64::MAX after
    // relative_age's saturating_sub) can't overflow the multiplication.
    let tenths = d.saturating_mul(10) / unit;
    let int_part = tenths / 10;
    if int_part >= 10 {
        format!("{int_part}{suffix}")
    } else {
        format!("{int_part}.{}{suffix}", tenths % 10)
    }
}

/// Lay out a run-start line's gutter: author `name` left-justified, `age`
/// right-justified, at least one space between, in exactly `width` columns. The
/// name claims the remaining columns (`width − 1 − age_width`) and is truncated to
/// fit, so the age stays flush-right and the separating rule column-aligns.
/// Measured in terminal display cells, not chars — CJK/emoji names are 2 cells
/// per char and would otherwise push the rule out of alignment.
pub fn gutter_token(name: &str, age: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    // Ages are ASCII (digits + unit suffix), but a corrupt timestamp can render
    // arbitrarily long — cap it to the column budget so the fixed-width gutter
    // contract holds even for garbage input.
    let age: String = age.chars().take(width).collect();
    let age_w = age.chars().count();
    let name_room = width.saturating_sub(1 + age_w);
    let mut name_trunc = String::new();
    let mut name_w = 0;
    for ch in name.chars() {
        let w = ch.width().unwrap_or(0);
        if name_w + w > name_room {
            break;
        }
        name_trunc.push(ch);
        name_w += w;
    }
    let gap = width.saturating_sub(name_w + age_w);
    format!("{name_trunc}{}{age}", " ".repeat(gap))
}

/// The blank gutter shown on a collapsed continuation line.
pub fn blank_gutter(width: usize) -> String {
    " ".repeat(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A delta that lands mid-bucket for `tenths`/10 of `unit` seconds, so the
    /// truncating formatter unambiguously reports exactly `tenths` (no boundary
    /// fragility, integer math only).
    fn tenths_ago(tenths: i64, unit: i64) -> i64 {
        (2 * tenths + 1) * unit / 20
    }

    #[test]
    fn age_hours_and_days_are_integers() {
        assert_eq!(relative_age(2 * HOUR, 0), "2h");
        assert_eq!(relative_age(23 * HOUR, 0), "23h");
        // 23h59m is still < 1 day → 23h, then exactly 1 day → 1d.
        assert_eq!(relative_age(DAY - 1, 0), "23h");
        assert_eq!(relative_age(DAY, 0), "1d");
        assert_eq!(relative_age(29 * DAY, 0), "29d");
    }

    #[test]
    fn age_months_single_digit_decimal_then_integer() {
        assert_eq!(relative_age(tenths_ago(25, MONTH), 0), "2.5m");
        assert_eq!(relative_age(tenths_ago(99, MONTH), 0), "9.9m");
        assert_eq!(relative_age(10 * MONTH, 0), "10m");
        assert_eq!(relative_age(11 * MONTH, 0), "11m");
    }

    #[test]
    fn age_twelve_months_rolls_to_one_year() {
        // Exactly 12 months is the year boundary and prints in years.
        assert_eq!(relative_age(12 * MONTH, 0), "1.0y");
        assert_eq!(relative_age(YEAR, 0), "1.0y");
        assert_eq!(relative_age(YEAR - 1, 0), "11m");
    }

    #[test]
    fn age_years_single_digit_decimal_then_integer() {
        assert_eq!(relative_age(tenths_ago(13, YEAR), 0), "1.3y");
        assert_eq!(relative_age(tenths_ago(99, YEAR), 0), "9.9y");
        assert_eq!(relative_age(10 * YEAR, 0), "10y");
        assert_eq!(relative_age(45 * YEAR, 0), "45y");
    }

    #[test]
    fn age_future_clamps_to_zero() {
        assert_eq!(relative_age(0, 100), "0h");
    }

    #[test]
    fn age_survives_a_corrupt_minimum_timestamp() {
        // A crafted/corrupt header time of i64::MIN saturates the subtraction to
        // i64::MAX; the ×10 must saturate too instead of overflowing (panic in
        // debug builds). The exact huge year count doesn't matter — only that it
        // formats as a sane integer-year token.
        let age = relative_age(0, i64::MIN);
        assert!(age.ends_with('y'), "extreme ages land in years: {age}");
        assert!(
            age.trim_end_matches('y').parse::<i64>().unwrap() > 0,
            "positive integer years: {age}"
        );
    }

    #[test]
    fn gutter_token_right_aligns_age_to_fixed_width() {
        // name shorter than its room → padded; age flush-right; total == width.
        let g = gutter_token("alice", "14d", GUTTER_W);
        assert_eq!(g.chars().count(), GUTTER_W);
        assert!(g.starts_with("alice"));
        assert!(g.ends_with("14d"));
    }

    #[test]
    fn gutter_token_truncates_a_long_name_to_keep_a_gap() {
        let g = gutter_token("sebastian", "2h", GUTTER_W);
        assert_eq!(g.chars().count(), GUTTER_W);
        assert!(g.ends_with("2h"));
        // Long name + short age: name room = 12 - 1 - 2 = 9, exactly "sebastian".
        assert!(g.starts_with("sebastian"));
        // A 4-col fractional age leaves only 7 cols for the name.
        let g2 = gutter_token("sebastian", "1.3y", GUTTER_W);
        assert_eq!(g2.chars().count(), GUTTER_W);
        assert!(g2.starts_with("sebasti"));
        assert!(g2.ends_with("1.3y"));
    }

    #[test]
    fn gutter_token_caps_a_runaway_age_to_the_column_budget() {
        // The worst age relative_age can emit (i64::MAX years, from a corrupt
        // near-i64::MIN timestamp) is exactly GUTTER_W chars — it must fill
        // the gutter, not widen it.
        let worst = relative_age(0, i64::MIN);
        assert!(worst.chars().count() <= GUTTER_W, "worst real age: {worst}");
        let g = gutter_token("bob", &worst, GUTTER_W);
        assert_eq!(g.chars().count(), GUTTER_W, "exactly the budget: {g:?}");
        // And any wider input (future formats, narrower gutters) is capped so
        // the fixed-width contract holds regardless.
        let g = gutter_token("bob", "1234567890123456y", GUTTER_W);
        assert_eq!(g.chars().count(), GUTTER_W, "oversized age capped: {g:?}");
    }

    #[test]
    fn gutter_token_measures_display_cells_not_chars() {
        use unicode_width::UnicodeWidthStr;
        // A CJK name is 2 cells per char: 田中太郎 = 4 chars / 8 cells. The token
        // must come out exactly GUTTER_W display cells so the ` │ ` rule aligns.
        let g = gutter_token("田中太郎", "2h", GUTTER_W);
        assert_eq!(g.width(), GUTTER_W, "display width, not char count: {g:?}");
        assert!(g.ends_with("2h"));
        // A longer CJK name truncates by cells (name room = 9 cols → 4 full-width
        // chars at 8 cells; a 5th would need 10).
        let g2 = gutter_token("田中太郎五", "2h", GUTTER_W);
        assert_eq!(g2.width(), GUTTER_W);
        assert!(g2.starts_with("田中太郎") && !g2.contains('五'));
    }

    #[test]
    fn blank_gutter_is_all_spaces() {
        assert_eq!(blank_gutter(GUTTER_W), " ".repeat(GUTTER_W));
    }
}
