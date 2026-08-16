//! Placement metrics for shaped lines and blocks — the value-only layer for
//! custom renderers that paint cached shaped lines directly instead of
//! composing elements.
//!
//! [`FontRhythm`] describes a text *style*; [`RhythmLineMetrics`] describes
//! one *actual shaped line*, whose `ascent`/`descent` come from the shaping
//! result (in gpui, the maximum over the line's explicit font runs). A line
//! mixing bold, italic, inline code, or explicit CJK/emoji fonts keeps one
//! baseline placed from those maxima, so only shaped metrics can land it on
//! the grid. [`RhythmBlockMetrics`] combines a line with the opening/closing
//! anchors into whole-row block geometry, including the fragment heights a
//! virtualized renderer needs.
//!
//! Everything here is a small `Copy` value: construction and every method are
//! O(1), allocation-free, and lock-free, and nothing touches a text system.

use crate::Rhythm;

/// Vertical metrics of one shaped line on the rhythm grid.
///
/// Unlike [`FontRhythm`](crate::FontRhythm), which carries a resolved *style*
/// (with its font size and optional cap/x heights), this is the minimal value
/// a renderer needs to place one already-shaped line: the line's real
/// `ascent`/`descent`, its height in whole rhythm units, and the grid. In
/// gpui, feed it `WrappedLine::ascent()` / `descent()` — the shaped maxima
/// over the line's explicit font runs — and every visual line of that
/// `WrappedLine` lands on the grid, because wrapped lines advance by the same
/// whole-row line height.
///
/// The renderer chooses `line_rhythms`; [`min_line_rhythms`](Self::min_line_rhythms)
/// suggests the smallest count whose line box contains the ink, and
/// [`overflows_line_box`](Self::overflows_line_box) reports when the chosen
/// count is smaller. Keeping a smaller count is valid — the baseline stays on
/// the grid and the ink overflows symmetrically via negative half-leading,
/// exactly as CSS line boxes behave.
///
/// A shaped *empty* line has zero ascent and descent; place empty lines with
/// the style's [`FontRhythm`](crate::FontRhythm) metrics instead so blank
/// lines keep the style's baseline position.
///
/// # Examples
///
/// ```
/// use rhythm_gpui::{Rhythm, RhythmLineMetrics};
///
/// let grid = Rhythm::new(8.0);
/// // A shaped line whose tallest run gives ascent 15.2, descent 4.1.
/// let line = RhythmLineMetrics::new(15.2, 4.1, 3, grid);
///
/// // Land its baseline on the 5th grid line below the block top: the paint
/// // origin is where the line box's top edge goes.
/// let origin_y = line.paint_origin_for(grid.height(5));
/// assert!((origin_y + line.baseline_above() - grid.height(5)).abs() < 1e-4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RhythmLineMetrics {
    ascent: f32,
    descent: f32,
    line_rhythms: u32,
    grid: Rhythm,
}

impl RhythmLineMetrics {
    /// Build from a shaped line's metrics: `ascent` and `descent` in logical
    /// pixels (both non-negative, e.g. gpui's `WrappedLine::ascent()` /
    /// `descent()`), and the line height in whole rhythm units.
    ///
    /// # Panics
    ///
    /// Panics when `ascent` or `descent` is negative or non-finite, or when
    /// `line_rhythms` is zero.
    pub fn new(ascent: f32, descent: f32, line_rhythms: u32, grid: Rhythm) -> Self {
        assert!(
            ascent.is_finite() && ascent >= 0.0,
            "ascent must be finite and non-negative"
        );
        assert!(
            descent.is_finite() && descent >= 0.0,
            "descent must be finite and non-negative"
        );
        assert!(line_rhythms > 0, "line_rhythms must be greater than zero");
        Self {
            ascent,
            descent,
            line_rhythms,
            grid,
        }
    }

    /// The shaped ascent above the baseline, non-negative.
    #[inline]
    pub const fn ascent(&self) -> f32 {
        self.ascent
    }

    /// The shaped descent below the baseline, non-negative.
    #[inline]
    pub const fn descent(&self) -> f32 {
        self.descent
    }

    /// Line height in whole rhythm units.
    #[inline]
    pub const fn line_rhythms(&self) -> u32 {
        self.line_rhythms
    }

    /// The grid the line is placed on.
    #[inline]
    pub const fn grid(&self) -> Rhythm {
        self.grid
    }

    /// The line height in logical pixels: `line_rhythms × grid size`.
    #[inline]
    pub fn line_height(&self) -> f32 {
        self.grid.size() * self.line_rhythms as f32
    }

    /// Extra space split above and below the `ascent + descent` box; negative
    /// when the ink is taller than the line box.
    #[inline]
    pub fn half_leading(&self) -> f32 {
        (self.line_height() - self.ascent - self.descent) / 2.0
    }

    /// Distance from the line box's top edge down to the baseline, as gpui
    /// paints it: `half_leading + ascent`.
    #[inline]
    pub fn baseline_above(&self) -> f32 {
        self.half_leading() + self.ascent
    }

    /// Distance from the baseline down to the line box's bottom edge.
    #[inline]
    pub fn baseline_below(&self) -> f32 {
        self.half_leading() + self.descent
    }

    /// Where to place the line box's top edge — the `origin.y` passed to
    /// gpui's `WrappedLine::paint` — so the baseline lands exactly on
    /// `target_baseline` (both in the same coordinate space):
    /// `target_baseline − baseline_above`.
    #[inline]
    pub fn paint_origin_for(&self, target_baseline: f32) -> f32 {
        target_baseline - self.baseline_above()
    }

    /// The smallest `line_rhythms` whose line box contains the shaped ink, at
    /// least 1. Shares [`Rhythm::snap_up`]'s float tolerance, so ink within a
    /// few rounding steps of a whole-row height does not claim an extra row.
    ///
    /// Advisory: the renderer decides whether to grow the line or keep its
    /// chosen height and accept the overflow.
    ///
    /// # Panics
    ///
    /// Panics when the minimum fitting count does not fit in `u32`.
    #[inline]
    pub fn min_line_rhythms(&self) -> u32 {
        let rows = self.minimum_line_rows();
        assert!(
            rows <= f64::from(u32::MAX),
            "minimum line rhythm count exceeds u32"
        );
        rows as u32
    }

    /// Whether the shaped ink is taller than the chosen line box —
    /// equivalently, whether [`half_leading`](Self::half_leading) is negative
    /// beyond [`Rhythm::snap_up`]'s tolerance.
    #[inline]
    pub fn overflows_line_box(&self) -> bool {
        self.minimum_line_rows() > f64::from(self.line_rhythms)
    }

    #[inline]
    fn minimum_line_rows(&self) -> f64 {
        let ink = f64::from(self.ascent) + f64::from(self.descent);
        let grid_size = f64::from(self.grid.size());
        let rows = ink / grid_size;
        let nearest = rows.round();
        let nearest_height = grid_size * nearest;
        let tolerance = ink * f64::from(f32::EPSILON) * 8.0;
        let snapped = if (ink - nearest_height).abs() <= tolerance {
            nearest
        } else {
            rows.ceil()
        };
        snapped.max(1.0)
    }
}

/// A text block on the rhythm grid as pure geometry: one line's metrics plus
/// the opening and closing anchors, with the fragment math a virtualized
/// renderer needs.
///
/// Two anchor modes, chosen at construction so an off-grid mixed pair is
/// inexpressible (the same guarantee `RhythmFont::cap_span` gives the
/// element-styled path):
///
/// - [`new`](Self::new) — baseline anchors: the block opens `top` rhythm
///   units above the first baseline and closes `bottom` units below the last
///   (the pure form of `rhythm_block`).
/// - [`cap`](Self::cap) — cap-ink anchors: the block opens `top` units above
///   the capitals' ink top and the closing returns the trimmed space (the
///   pure form of `cap_top` / `cap_bottom`).
///
/// Either way the block spans a whole number of rhythm rows for any number of
/// lines ([`rows`](Self::rows) is exact integer arithmetic), so fragments and
/// whole blocks compose without breaking the page rhythm.
///
/// # Fragments
///
/// A block split at visual-line boundaries keeps its rhythm phase because
/// every line advances by whole rows: stack
/// [`first_height`](Self::first_height) + [`middle_height`](Self::middle_height)
/// × N + [`last_height`](Self::last_height) and every baseline lands where
/// the unsplit block would have put it. The first baseline sits
/// [`first_baseline`](Self::first_baseline) below the block top in a
/// first/single fragment, and [`RhythmLineMetrics::baseline_above`] below the
/// fragment top in a middle/last fragment.
///
/// # Examples
///
/// ```
/// use rhythm_gpui::{Rhythm, RhythmBlockMetrics, RhythmLineMetrics};
///
/// let grid = Rhythm::new(8.0);
/// // Georgia-like 16px body on a 3-unit (24px) line.
/// let line = RhythmLineMetrics::new(14.67, 3.51, 3, grid);
/// let block = RhythmBlockMetrics::new(line, 3, 1);
///
/// // Five wrapped lines: the block covers whole rhythm rows…
/// let height = block.height(5);
/// assert_eq!(block.rows(5), 16);
/// assert!((height - grid.height(16)).abs() < 1e-3);
///
/// // …and splitting after two lines preserves both height and phase.
/// let split = block.first_height(2) + block.last_height(3);
/// assert!((split - height).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RhythmBlockMetrics {
    line: RhythmLineMetrics,
    top: i32,
    bottom: i32,
    cap_height: Option<f32>,
}

impl RhythmBlockMetrics {
    /// A baseline-anchored block: opens `top` rhythm units above the first
    /// baseline, closes `bottom` units below the last. Negative counts are
    /// meaningful for margin-style layouts, matching `baseline_top` /
    /// `baseline_bottom`.
    pub const fn new(line: RhythmLineMetrics, top: i32, bottom: i32) -> Self {
        Self {
            line,
            top,
            bottom,
            cap_height: None,
        }
    }

    /// A cap-anchored block: opens `top` rhythm units above the capitals'
    /// ink top and closes with the trimmed space returned, so the block still
    /// spans whole rows — the pure form of the `cap_top` / `cap_bottom` pair.
    /// Baselines shift off the grid by `cap_height mod grid size`, exactly as
    /// the element-styled cap opening does.
    ///
    /// `cap_height` is the style's capital height above the baseline
    /// (`FontRhythm::cap_height`); shaped lines do not carry one.
    ///
    /// # Panics
    ///
    /// Panics when `cap_height` is zero, negative, or non-finite.
    pub fn cap(line: RhythmLineMetrics, cap_height: f32, top: i32, bottom: i32) -> Self {
        assert!(
            cap_height.is_finite() && cap_height > 0.0,
            "cap height must be finite and greater than zero"
        );
        Self {
            line,
            top,
            bottom,
            cap_height: Some(cap_height),
        }
    }

    /// The line metrics the block is set in.
    #[inline]
    pub const fn line(&self) -> RhythmLineMetrics {
        self.line
    }

    /// Opening rhythm units above the anchor (baseline or cap ink).
    #[inline]
    pub const fn top(&self) -> i32 {
        self.top
    }

    /// Closing rhythm units below the last baseline (or the paired cap close).
    #[inline]
    pub const fn bottom(&self) -> i32 {
        self.bottom
    }

    /// Whether the block uses cap-ink anchors instead of baseline anchors.
    #[inline]
    pub const fn is_cap_anchored(&self) -> bool {
        self.cap_height.is_some()
    }

    fn grid(&self) -> Rhythm {
        self.line.grid()
    }

    /// Space from the block's top edge down to the first line box —
    /// `baseline_top(top)` for baseline anchors, `cap_top(top)` for cap
    /// anchors. Negative when `top` is smaller than the anchor distance;
    /// meaningful as a margin, not as a padding.
    #[inline]
    pub fn opening(&self) -> f32 {
        let above = self.line.baseline_above();
        match self.cap_height {
            None => self.grid().height(self.top) - above,
            Some(cap) => self.grid().height(self.top) - (above - cap),
        }
    }

    /// Space from the last line box down to the block's bottom edge — the
    /// counterpart of [`opening`](Self::opening) that closes the block on
    /// whole rows.
    #[inline]
    pub fn closing(&self) -> f32 {
        match self.cap_height {
            None => self.grid().height(self.bottom) - self.line.baseline_below(),
            Some(cap) => self.grid().height(self.bottom) + (self.line.baseline_above() - cap),
        }
    }

    /// Distance from the block's top edge down to the first baseline:
    /// `top` whole rhythm units for baseline anchors, plus the cap height for
    /// cap anchors. Applies to first and single fragments; in a middle/last
    /// fragment the first baseline sits [`RhythmLineMetrics::baseline_above`]
    /// below the fragment top.
    #[inline]
    pub fn first_baseline(&self) -> f32 {
        self.grid().height(self.top) + self.cap_height.unwrap_or(0.0)
    }

    /// Height of the whole block (a single fragment) containing `lines`
    /// visual lines: opening + lines + closing.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn height(&self, lines: u32) -> f32 {
        self.first_height(lines) + self.closing()
    }

    /// Height of a first fragment: the opening plus `lines` visual lines,
    /// split before the next line box.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn first_height(&self, lines: u32) -> f32 {
        self.opening() + self.middle_height(lines)
    }

    /// Height of a middle fragment: `lines` whole line boxes, nothing else.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn middle_height(&self, lines: u32) -> f32 {
        assert!(lines > 0, "a fragment must contain at least one line");
        self.line.line_height() * lines as f32
    }

    /// Height of a last fragment: `lines` visual lines plus the closing.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn last_height(&self, lines: u32) -> f32 {
        self.middle_height(lines) + self.closing()
    }

    /// The whole number of rhythm rows a single-fragment block of `lines`
    /// visual lines covers, as exact integer arithmetic:
    /// `top + bottom + (lines − 1) × line_rhythms` for baseline anchors,
    /// `top + bottom + lines × line_rhythms` for cap anchors (cap spacing is
    /// measured from ink, not baseline, so the extra baseline distance stays
    /// inside the block).
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or the resulting row count does not fit in
    /// `i32`.
    #[inline]
    pub fn rows(&self, lines: u32) -> i32 {
        assert!(lines > 0, "a fragment must contain at least one line");
        let spanned = if self.cap_height.is_some() {
            lines
        } else {
            lines - 1
        };
        let rows = i128::from(self.top)
            + i128::from(self.bottom)
            + i128::from(spanned) * i128::from(self.line.line_rhythms());
        i32::try_from(rows).expect("block row count exceeds i32")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontRhythm;

    const GRID: Rhythm = Rhythm::new(8.0);

    // Georgia on macOS at 16px: upem 2048, hhea 1878/-449, cap 1419.
    fn georgia_line() -> RhythmLineMetrics {
        RhythmLineMetrics::new(16.0 * 1878.0 / 2048.0, 16.0 * 449.0 / 2048.0, 3, GRID)
    }

    fn georgia_font() -> FontRhythm {
        FontRhythm::from_platform_metrics(
            16.0,
            3,
            16.0 * 1878.0 / 2048.0,
            -16.0 * 449.0 / 2048.0,
            16.0 * 1419.0 / 2048.0,
            16.0 * 986.0 / 2048.0,
        )
    }

    #[test]
    fn line_metrics_agree_with_the_font_model() {
        // Same metrics through both types: the crate's one placement formula.
        let line = georgia_line();
        let font = georgia_font();
        assert!((line.line_height() - font.line_height(GRID)).abs() < 1e-6);
        assert!((line.half_leading() - font.half_leading(GRID)).abs() < 1e-6);
        assert!((line.baseline_above() - font.baseline_above(GRID)).abs() < 1e-6);
        assert!((line.baseline_below() - font.baseline_below(GRID)).abs() < 1e-6);
    }

    #[test]
    fn paint_origin_solves_the_renderer_baseline_formula() {
        // gpui paints the baseline at origin + (line_height - ascent -
        // descent) / 2 + ascent; the origin must invert exactly that.
        let line = georgia_line();
        let target = GRID.height(5);
        let origin = line.paint_origin_for(target);
        let painted =
            origin + (line.line_height() - line.ascent() - line.descent()) / 2.0 + line.ascent();
        assert!((painted - target).abs() < 1e-4);
    }

    #[test]
    fn min_line_rhythms_covers_the_ink() {
        // Georgia 16px ink is ~18.2px: three 8px rows minimum.
        let line = georgia_line();
        assert_eq!(line.min_line_rhythms(), 3);
        assert!(!line.overflows_line_box());

        // An emoji-tall run on the same 3-row box overflows; four rows fit.
        let tall = RhythmLineMetrics::new(20.0, 6.0, 3, GRID);
        assert_eq!(tall.min_line_rhythms(), 4);
        assert!(tall.overflows_line_box());
        assert!(tall.half_leading() < 0.0);
        let grown = RhythmLineMetrics::new(20.0, 6.0, 4, GRID);
        assert!(!grown.overflows_line_box());

        // Exactly filling ink is not an overflow, and float error within
        // snap_up's tolerance does not claim an extra row.
        let exact = RhythmLineMetrics::new(20.0, 4.0, 3, GRID);
        assert_eq!(exact.min_line_rhythms(), 3);
        assert!(!exact.overflows_line_box());
        let noisy = RhythmLineMetrics::new(20.000_001, 4.0, 3, GRID);
        assert_eq!(noisy.min_line_rhythms(), 3);
    }

    #[test]
    fn line_height_preserves_the_full_u32_count() {
        let count = i32::MAX as u32 + 1;
        let line = RhythmLineMetrics::new(1.0, 1.0, count, GRID);
        assert_eq!(line.line_height(), GRID.size() * count as f32);
        assert!(line.line_height().is_sign_positive());
    }

    #[test]
    fn overflow_is_reported_when_no_u32_line_count_can_fit_the_ink() {
        let tiny_grid = Rhythm::new(f32::MIN_POSITIVE);
        let line = RhythmLineMetrics::new(1.0, 0.0, u32::MAX, tiny_grid);
        assert!(line.overflows_line_box());
    }

    #[test]
    #[should_panic(expected = "minimum line rhythm count exceeds u32")]
    fn minimum_line_count_rejects_a_value_outside_the_return_type() {
        let tiny_grid = Rhythm::new(f32::MIN_POSITIVE);
        let line = RhythmLineMetrics::new(1.0, 0.0, 1, tiny_grid);
        let _ = line.min_line_rhythms();
    }

    #[test]
    fn empty_shaped_lines_are_representable() {
        let empty = RhythmLineMetrics::new(0.0, 0.0, 3, GRID);
        assert_eq!(empty.min_line_rhythms(), 1);
        assert!(!empty.overflows_line_box());
        assert!((empty.baseline_above() - GRID.height(3) / 2.0).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "ascent must be finite and non-negative")]
    fn line_metrics_reject_a_negative_ascent() {
        let _ = RhythmLineMetrics::new(-1.0, 3.0, 3, GRID);
    }

    #[test]
    #[should_panic(expected = "line_rhythms must be greater than zero")]
    fn line_metrics_reject_zero_rhythms() {
        let _ = RhythmLineMetrics::new(15.0, 4.0, 0, GRID);
    }

    #[test]
    fn baseline_anchors_match_the_style_level_spacing() {
        let block = RhythmBlockMetrics::new(georgia_line(), 3, 1);
        let font = georgia_font();
        assert!((block.opening() - GRID.baseline_top(&font, 3)).abs() < 1e-6);
        assert!((block.closing() - GRID.baseline_bottom(&font, 1)).abs() < 1e-6);
        assert!((block.first_baseline() - GRID.height(3)).abs() < 1e-6);
        assert!(!block.is_cap_anchored());
    }

    #[test]
    fn cap_anchors_match_the_style_level_spacing() {
        let font = georgia_font();
        let cap_height = font.cap_height().unwrap();
        let block = RhythmBlockMetrics::cap(georgia_line(), cap_height, 3, 0);
        assert!((block.opening() - GRID.cap_top(&font, 3).unwrap()).abs() < 1e-6);
        assert!((block.closing() - GRID.cap_bottom(&font, 0).unwrap()).abs() < 1e-6);
        assert!((block.first_baseline() - (GRID.height(3) + cap_height)).abs() < 1e-6);
        assert!(block.is_cap_anchored());
    }

    #[test]
    fn blocks_cover_whole_rows_and_rows_is_exact() {
        let block = RhythmBlockMetrics::new(georgia_line(), 3, 1);
        for lines in [1, 2, 5, 40] {
            let height = block.height(lines);
            let rows = block.rows(lines);
            assert_eq!(rows, 3 + 1 + (lines as i32 - 1) * 3);
            assert!((height - GRID.height(rows)).abs() < 1e-3);
        }

        let cap = RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0);
        for lines in [1, 2, 5] {
            let rows = cap.rows(lines);
            assert_eq!(rows, 3 + lines as i32 * 3);
            assert!((cap.height(lines) - GRID.height(rows)).abs() < 1e-3);
        }
    }

    #[test]
    #[should_panic(expected = "block row count exceeds i32")]
    fn rows_reject_a_count_outside_the_return_type() {
        let line = RhythmLineMetrics::new(1.0, 1.0, u32::MAX, GRID);
        let _ = RhythmBlockMetrics::new(line, 0, 0).rows(2);
    }

    #[test]
    fn fragments_sum_to_the_block_and_keep_the_phase() {
        let block = RhythmBlockMetrics::new(georgia_line(), 3, 1);
        let split = block.first_height(2) + block.middle_height(4) + block.last_height(3);
        assert!((split - block.height(9)).abs() < 1e-3);

        // With the block top on a grid line, every continuation fragment's
        // first baseline still lands on the grid.
        let above = block.line().baseline_above();
        let middle_top = block.first_height(2);
        let last_top = middle_top + block.middle_height(4);
        for top in [middle_top, last_top] {
            let baseline = top + above;
            let rows = baseline / GRID.size();
            assert!((rows - rows.round()).abs() < 1e-3, "off grid: {baseline}");
        }
    }

    #[test]
    fn negative_anchor_counts_stay_meaningful_as_margins() {
        let block = RhythmBlockMetrics::new(georgia_line(), 0, -1);
        assert!(block.opening() < 0.0);
        assert!(block.closing() < 0.0);
        assert_eq!(block.rows(2), 0 - 1 + 3);
    }

    #[test]
    #[should_panic(expected = "at least one line")]
    fn fragments_reject_zero_lines() {
        let _ = RhythmBlockMetrics::new(georgia_line(), 3, 1).height(0);
    }

    #[test]
    #[should_panic(expected = "cap height must be finite and greater than zero")]
    fn cap_blocks_reject_an_unusable_cap_height() {
        let _ = RhythmBlockMetrics::cap(georgia_line(), 0.0, 3, 0);
    }
}
