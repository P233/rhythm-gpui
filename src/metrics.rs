//! Placement metrics for shaped lines and blocks — the value-only layer for
//! custom renderers that paint cached shaped lines directly instead of
//! composing elements.
//!
//! [`FontRhythm`] describes a text *style*; [`RhythmLineMetrics`] describes
//! one *actual shaped line*, whose `ascent`/`descent` come from the shaping
//! result (in gpui, the maximum over the line's explicit font runs). A line
//! mixing bold, italic, inline code, or explicit CJK/emoji fonts keeps one
//! baseline placed from those maxima, so only shaped metrics can land it on
//! the grid. [`RhythmBlockMetrics`] combines a line with baseline or cap-ink
//! anchors into whole-row block geometry, with the exact row arithmetic a
//! virtualized renderer needs.
//!
//! Everything here is a small `Copy` value. Construction and scalar geometry
//! methods are O(1); [`RhythmLineMetrics::covering`] is O(`metrics.len()`). All
//! of them are allocation-free and lock-free, and nothing touches a text
//! system.

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
/// exactly as CSS line boxes behave. [`covering`](Self::covering) picks the
/// count the other way round: over a known set of faces, before anything is
/// shaped.
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
    #[inline]
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

    /// Like [`new`](Self::new), but grows the line box to
    /// [`min_line_rhythms`](Self::min_line_rhythms) when `line_rhythms` is too
    /// small to contain the shaped ink. Use it when the configured line height
    /// is a floor rather than a fixed virtualization budget.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`new`](Self::new), or when the
    /// minimum fitting count does not fit in `u32`.
    pub fn at_least(ascent: f32, descent: f32, line_rhythms: u32, grid: Rhythm) -> Self {
        let line = Self::new(ascent, descent, line_rhythms, grid);
        Self {
            line_rhythms: line_rhythms.max(line.min_line_rhythms()),
            ..line
        }
    }

    /// The smallest line box on `grid` containing every line in `metrics`:
    /// the maximum ascent and the maximum descent over the set, in the
    /// largest of their line heights, grown to
    /// [`min_line_rhythms`](Self::min_line_rhythms) when that combined ink no
    /// longer fits.
    ///
    /// A line shapes to the maxima over its explicit font runs, so no line
    /// drawn from a known set of faces can exceed the box covering that set.
    /// Folding a style's whole set — its own face, the faces its runs use
    /// (bold, inline code, an explicit CJK or emoji face), and the families
    /// gpui would resolve to if one were missing — at catalog-build time
    /// makes "the line box contains the ink" a property of construction
    /// rather than one each shaped line has to be checked for. Nothing here
    /// shapes text: the count is known at startup, so a block's height
    /// follows from its line count alone, which is what a virtualized
    /// renderer needs.
    ///
    /// Take the count, not the box, into placement: this value's
    /// `ascent`/`descent` describe a hypothetical line reaching both maxima
    /// at once, so keep building each line's metrics from its own shaped
    /// values — at this [`line_rhythms`](Self::line_rhythms) — and every
    /// baseline still lands on the grid.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhythm_gpui::{Rhythm, RhythmLineMetrics};
    ///
    /// let grid = Rhythm::new(8.0);
    /// // A body style, and a display face its lines can mix in.
    /// let body = RhythmLineMetrics::new(14.67, 3.51, 3, grid);
    /// let display = RhythmLineMetrics::new(28.8, 6.4, 3, grid);
    ///
    /// // One static row budget for the style: three rows cannot hold the
    /// // display face's ink, so the covering box is five.
    /// let budget = RhythmLineMetrics::covering(&[body, display], grid);
    /// assert_eq!(budget.line_rhythms(), 5);
    ///
    /// // Every mixture of the two fits it, including the worst case.
    /// let worst = RhythmLineMetrics::new(28.8, 6.4, budget.line_rhythms(), grid);
    /// assert!(!worst.overflows_line_box());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when `metrics` is empty, when an entry was built on a
    /// different grid, or when the minimum fitting count does not fit in
    /// `u32`.
    pub fn covering(metrics: &[RhythmLineMetrics], grid: Rhythm) -> Self {
        assert!(
            !metrics.is_empty(),
            "a covering line box must cover at least one line"
        );
        let mut ascent = 0.0;
        let mut descent = 0.0;
        let mut line_rhythms = 1;
        for line in metrics {
            assert_eq!(
                line.grid, grid,
                "every covered line must be built on the covering grid"
            );
            ascent = f32::max(ascent, line.ascent);
            descent = f32::max(descent, line.descent);
            line_rhythms = line_rhythms.max(line.line_rhythms);
        }
        Self::at_least(ascent, descent, line_rhythms, grid)
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
    /// least 1. Applies the same snapping rule as [`Rhythm::snap_up`] — at
    /// `f64` precision, since shaped metrics are summed rather than measured —
    /// so ink within a few rounding steps of a whole-row height does not claim
    /// an extra row.
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

    // The same snapping rule as `Rhythm::snap_rows`, computed in `f64`: the
    // inputs are exact `f32` promotions, so here the division noise sits far
    // below the tolerance instead of being what it absorbs. The duplication
    // is deliberate — see the note there before merging them.
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
/// baseline or cap-ink anchors, with both fragment geometry and the exact row
/// arithmetic a virtualized renderer needs. [`new`](Self::new) is the pure
/// form of `rhythm_block`; [`cap`](Self::cap) pairs `cap_top` with `cap_bottom`.
/// Either mode spans a whole number of rhythm rows for any number of lines, so
/// blocks and fragments compose without breaking the page rhythm.
///
/// # Fragments
///
/// A block split at visual-line boundaries keeps its rhythm phase because
/// every line advances by whole rows. The `first` / `middle` / `last` height
/// methods expose concrete fragment geometry, while [`first_rows`](Self::first_rows),
/// [`middle_rows`](Self::middle_rows), and [`last_rows`](Self::last_rows) expose
/// exact `i32` cursor deltas. A virtualizer can accumulate those deltas in an
/// `i64`, rebase near the viewport, then derive only a visible fragment's
/// baseline with [`baseline_at_row`](Self::baseline_at_row) and line-box top
/// with [`RhythmLineMetrics::paint_origin_for`].
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
/// // Five wrapped lines cover sixteen whole rhythm rows…
/// assert_eq!(block.rows(5), 3 + 1 + 4 * 3);
///
/// // …and the first baseline sits `top` grid lines below the block top.
/// assert_eq!(block.first_baseline(), grid.height(3));
/// let origin_y = line.paint_origin_for(block.first_baseline());
/// assert!((origin_y + line.baseline_above() - grid.height(3)).abs() < 1e-4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RhythmBlockMetrics {
    line: RhythmLineMetrics,
    top: i32,
    bottom: i32,
    cap_height: Option<f32>,
}

impl RhythmBlockMetrics {
    /// A block opening `top` rhythm units above the first baseline and
    /// closing `bottom` units below the last. Negative counts are meaningful
    /// for margin-style layouts, matching `baseline_top` / `baseline_bottom`.
    pub const fn new(line: RhythmLineMetrics, top: i32, bottom: i32) -> Self {
        Self {
            line,
            top,
            bottom,
            cap_height: None,
        }
    }

    /// A cap-anchored block: opens `top` rhythm units above the capitals' ink
    /// top and closes with the paired trimmed space, retaining the cap phase
    /// across fragments while the whole block still spans integer rows.
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

    /// Opening rhythm units above the anchor (baseline or cap ink). A row
    /// count, not a length — [`opening`](Self::opening) is the pixel spacing
    /// it produces.
    #[inline]
    pub const fn top_rhythms(&self) -> i32 {
        self.top
    }

    /// Closing rhythm units below the last baseline, or the paired cap close.
    /// A row count, not a length; see [`closing`](Self::closing).
    #[inline]
    pub const fn bottom_rhythms(&self) -> i32 {
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

    /// Space from the block's top edge down to its first line box. Negative
    /// values are meaningful as margins rather than padding.
    #[inline]
    pub fn opening(&self) -> f32 {
        let above = self.line.baseline_above();
        match self.cap_height {
            None => self.grid().height(self.top) - above,
            Some(cap) => self.grid().height(self.top) - (above - cap),
        }
    }

    /// Space from the last line box down to the block's whole-row bottom edge.
    #[inline]
    pub fn closing(&self) -> f32 {
        match self.cap_height {
            None => self.grid().height(self.bottom) - self.line.baseline_below(),
            Some(cap) => self.grid().height(self.bottom) + (self.line.baseline_above() - cap),
        }
    }

    /// Distance from the block's top edge down to the first baseline: `top`
    /// whole rhythm units for baseline anchors, plus the cap height for cap
    /// anchors. Applies to first and single fragments; in a middle/last
    /// fragment the first baseline sits
    /// [`RhythmLineMetrics::baseline_above`] below the fragment top.
    #[inline]
    pub fn first_baseline(&self) -> f32 {
        self.baseline_at_row(i64::from(self.top))
    }

    /// Height of the whole block containing `lines` visual lines.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn height(&self, lines: u32) -> f32 {
        self.first_height(lines) + self.closing()
    }

    /// Height of a first fragment: opening plus `lines` whole line boxes.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn first_height(&self, lines: u32) -> f32 {
        self.opening() + self.middle_height(lines)
    }

    /// Height of a middle fragment containing `lines` whole line boxes.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero.
    #[inline]
    pub fn middle_height(&self, lines: u32) -> f32 {
        assert!(lines > 0, "a fragment must contain at least one line");
        self.line.line_height() * lines as f32
    }

    /// Height of a last fragment: `lines` whole line boxes plus the closing.
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
    /// `top + bottom + (lines − 1) × line_rhythms` for baseline anchors, or
    /// `top + bottom + lines × line_rhythms` for cap anchors. The block's
    /// height in pixels is `grid.height(rows(lines))`.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or the resulting row count does not fit in
    /// `i32`.
    #[inline]
    pub fn rows(&self, lines: u32) -> i32 {
        checked_rows(
            i128::from(self.top) + self.spanned_rows(lines) + i128::from(self.bottom)
                - self.trailing_rows(),
        )
    }

    /// Row-cursor delta from the block top to the baseline immediately after a
    /// *first* fragment of `lines` visual lines. Add it to the block's starting
    /// row, then pass the result to [`baseline_at_row`](Self::baseline_at_row)
    /// to place the first line of the next fragment.
    ///
    /// This and its two siblings partition [`rows`](Self::rows) exactly:
    /// `first_rows(a) + middle_rows(b) + last_rows(c) == rows(a + b + c)`.
    /// These values are cursor transitions, not fragment heights converted to
    /// rows: continuation fragments start at a line-box top, generally between
    /// grid lines. Keeping the cursor as rows and deriving only the visible
    /// fragment's baseline prevents accumulated floating-point drift.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhythm_gpui::{Rhythm, RhythmBlockMetrics, RhythmLineMetrics};
    ///
    /// let grid = Rhythm::new(8.0);
    /// let line = RhythmLineMetrics::new(14.67, 3.51, 3, grid);
    /// let block = RhythmBlockMetrics::new(line, 3, 1);
    ///
    /// // A nine-line block split 2 / 4 / 3 across three viewport pages: the
    /// // middle fragment starts two line advances below the first line box.
    /// let first_origin = line.paint_origin_for(block.first_baseline());
    /// let mut cursor = i64::from(block.first_rows(2));
    /// let middle_origin = line.paint_origin_for(block.baseline_at_row(cursor));
    /// assert!((middle_origin - (first_origin + 2.0 * line.line_height())).abs() < 1e-3);
    /// cursor = cursor.checked_add(i64::from(block.middle_rows(4))).unwrap();
    /// cursor = cursor.checked_add(i64::from(block.last_rows(3))).unwrap();
    /// assert_eq!(cursor, i64::from(block.rows(9)));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or the row count does not fit in `i32`.
    #[inline]
    pub fn first_rows(&self, lines: u32) -> i32 {
        checked_rows(i128::from(self.top) + self.spanned_rows(lines))
    }

    /// Row-cursor delta across a *middle* fragment of `lines` visual lines:
    /// `lines × line_rhythms`. The cursor points to the fragment's first
    /// baseline before this delta and the following fragment's first baseline
    /// after it. See [`first_rows`](Self::first_rows) for the full contract.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or the row count does not fit in `i32`.
    #[inline]
    pub fn middle_rows(&self, lines: u32) -> i32 {
        checked_rows(self.spanned_rows(lines))
    }

    /// The baseline at an accumulated `i64` grid-row cursor. Baseline-anchored
    /// blocks land on `row × grid size`; cap-anchored blocks retain their cap
    /// phase.
    ///
    /// Keep the cursor as exact integer rows and rebase it near the viewport
    /// before this conversion. The returned coordinate is `f32`, so accepting
    /// `i64` avoids an arbitrary saturating cast but cannot make enormous
    /// absolute pixel coordinates exact.
    ///
    /// Pass this to [`RhythmLineMetrics::paint_origin_for`] to derive the
    /// fragment's line-box top without accumulating preceding `f32` heights.
    #[inline]
    pub fn baseline_at_row(&self, row: i64) -> f32 {
        self.grid().size() * row as f32 + self.cap_height.unwrap_or(0.0)
    }

    /// Row-cursor delta from the first baseline of a *last* fragment of
    /// `lines` visual lines to the block's whole-row bottom edge: `lines − 1`
    /// line advances plus the closing for baseline anchors, or `lines`
    /// advances plus the paired cap close for cap anchors. See
    /// [`first_rows`](Self::first_rows) for the full contract.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or the row count does not fit in `i32`.
    #[inline]
    pub fn last_rows(&self, lines: u32) -> i32 {
        checked_rows(self.spanned_rows(lines) + i128::from(self.bottom) - self.trailing_rows())
    }

    #[inline]
    fn trailing_rows(&self) -> i128 {
        if self.cap_height.is_some() {
            0
        } else {
            i128::from(self.line.line_rhythms())
        }
    }

    #[inline]
    fn spanned_rows(&self, lines: u32) -> i128 {
        assert!(lines > 0, "a fragment must contain at least one line");
        i128::from(lines) * i128::from(self.line.line_rhythms())
    }
}

#[inline]
fn checked_rows(rows: i128) -> i32 {
    i32::try_from(rows).expect("block row count exceeds i32")
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
    fn at_least_grows_only_an_overflowing_line_box() {
        let grown = RhythmLineMetrics::at_least(20.0, 6.0, 3, GRID);
        assert_eq!(grown.line_rhythms(), 4);
        assert!(!grown.overflows_line_box());

        let roomy = RhythmLineMetrics::at_least(20.0, 6.0, 6, GRID);
        assert_eq!(roomy.line_rhythms(), 6);
        assert_eq!(
            RhythmLineMetrics::at_least(georgia_line().ascent(), georgia_line().descent(), 3, GRID,),
            georgia_line()
        );
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
        // The direct-paint placement and the element-path paddings are one
        // formula: the first line box's top sits `baseline_top(3)` below the
        // block top, and the whole-row block closes `baseline_bottom(1)`
        // under the last line box.
        let line = georgia_line();
        let block = RhythmBlockMetrics::new(line, 3, 1);
        let font = georgia_font();
        assert!((block.first_baseline() - GRID.height(3)).abs() < 1e-6);
        let first_origin = line.paint_origin_for(block.first_baseline());
        assert!((first_origin - GRID.baseline_top(&font, 3)).abs() < 1e-6);
        let closing = GRID.height(block.rows(1)) - (first_origin + line.line_height());
        assert!((closing - GRID.baseline_bottom(&font, 1)).abs() < 1e-6);
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
    fn rows_is_exact_integer_arithmetic() {
        let block = RhythmBlockMetrics::new(georgia_line(), 3, 1);
        for lines in [1, 2, 5, 40] {
            assert_eq!(block.rows(lines), 3 + 1 + (lines as i32 - 1) * 3);
            assert!((block.height(lines) - GRID.height(block.rows(lines))).abs() < 1e-3);
        }

        let cap = RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0);
        for lines in [1, 2, 5] {
            assert_eq!(cap.rows(lines), 3 + lines as i32 * 3);
            assert!((cap.height(lines) - GRID.height(cap.rows(lines))).abs() < 1e-3);
        }
    }

    #[test]
    fn fragment_heights_sum_to_the_whole_block() {
        for block in [
            RhythmBlockMetrics::new(georgia_line(), 3, 1),
            RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0),
        ] {
            let split = block.first_height(2) + block.middle_height(4) + block.last_height(3);
            assert!((split - block.height(9)).abs() < 1e-3);
        }
    }

    #[test]
    #[should_panic(expected = "block row count exceeds i32")]
    fn rows_reject_a_count_outside_the_return_type() {
        let line = RhythmLineMetrics::new(1.0, 1.0, u32::MAX, GRID);
        let _ = RhythmBlockMetrics::new(line, 0, 0).rows(2);
    }

    #[test]
    fn fragment_rows_partition_the_block_exactly() {
        for block in [
            RhythmBlockMetrics::new(georgia_line(), 3, 1),
            RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0),
            RhythmBlockMetrics::new(georgia_line(), -2, 1),
            RhythmBlockMetrics::cap(georgia_line(), 11.09, -2, -1),
        ] {
            let line_rhythms = block.line().line_rhythms() as i32;
            assert_eq!(
                block.first_rows(2) + block.middle_rows(4) + block.last_rows(3),
                block.rows(9)
            );
            assert_eq!(block.first_rows(1) + block.last_rows(1), block.rows(2));
            assert_eq!(block.first_rows(5), block.top_rhythms() + 5 * line_rhythms);
            assert_eq!(block.middle_rows(4), 4 * line_rhythms);
            let trailing = if block.is_cap_anchored() {
                0
            } else {
                line_rhythms
            };
            assert_eq!(
                block.last_rows(3),
                3 * line_rhythms + block.bottom_rhythms() - trailing
            );
        }
    }

    #[test]
    fn fragment_row_cursor_reconstructs_every_continuation_origin() {
        for block in [
            RhythmBlockMetrics::new(georgia_line(), 3, 1),
            RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0),
            RhythmBlockMetrics::new(georgia_line(), -2, 1),
            RhythmBlockMetrics::cap(georgia_line(), 11.09, -2, -1),
        ] {
            let line = block.line();
            // Split 2 / 4 / 1 / 3 / 2: each continuation fragment's line-box
            // top must land where the unsplit block's next line box would —
            // whole line advances below the first line box.
            let mut rows = i64::from(block.first_rows(2));
            let mut expected =
                line.paint_origin_for(block.first_baseline()) + 2.0 * line.line_height();
            for lines in [4, 1, 3] {
                let origin = line.paint_origin_for(block.baseline_at_row(rows));
                assert!((origin - expected).abs() < 1e-3);
                rows = rows
                    .checked_add(i64::from(block.middle_rows(lines)))
                    .expect("test row cursor overflowed");
                expected += line.line_height() * lines as f32;
            }

            let last_origin = line.paint_origin_for(block.baseline_at_row(rows));
            assert!((last_origin - expected).abs() < 1e-3);
            rows = rows
                .checked_add(i64::from(block.last_rows(2)))
                .expect("test row cursor overflowed");
            assert_eq!(rows, i64::from(block.rows(2 + 4 + 1 + 3 + 2)));
        }
    }

    #[test]
    fn baseline_at_row_accepts_a_wide_cursor_without_i32_saturation() {
        let block = RhythmBlockMetrics::new(georgia_line(), 3, 1);
        let wide_row = i64::from(i32::MAX) * 4;
        assert_eq!(
            block.baseline_at_row(wide_row),
            GRID.size() * wide_row as f32
        );
    }

    #[test]
    fn virtualizer_rebases_a_wide_multi_block_cursor_before_float_conversion() {
        let preceding = RhythmBlockMetrics::new(georgia_line(), 3, 1);
        let visible = RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0);

        let mut document_cursor = i64::from(i32::MAX) * 4;
        document_cursor = document_cursor
            .checked_add(i64::from(preceding.rows(40)))
            .expect("test document cursor overflowed");
        let visible_top = document_cursor;
        document_cursor = document_cursor
            .checked_add(i64::from(visible.rows(9)))
            .expect("test document cursor overflowed");
        assert!(document_cursor > i64::from(i32::MAX));

        // The viewport starts two rows before the visible block. Only these
        // small relative cursors cross the i64 -> f32 boundary, so adjacent
        // baselines remain exact even though the document cursor is huge.
        let viewport_origin = visible_top
            .checked_sub(2)
            .expect("test viewport origin overflowed");
        let first_row = visible_top
            .checked_add(i64::from(visible.top_rhythms()))
            .and_then(|row| row.checked_sub(viewport_origin))
            .expect("test first baseline cursor overflowed");
        let continuation_row = visible_top
            .checked_add(i64::from(visible.first_rows(2)))
            .and_then(|row| row.checked_sub(viewport_origin))
            .expect("test continuation cursor overflowed");

        let first_baseline = visible.baseline_at_row(first_row);
        let continuation_baseline = visible.baseline_at_row(continuation_row);
        assert!((first_baseline - (GRID.height(2) + visible.first_baseline())).abs() < 1e-6);
        assert!(
            (continuation_baseline - first_baseline - 2.0 * visible.line().line_height()).abs()
                < 1e-4
        );
    }

    #[test]
    fn baseline_at_row_preserves_cap_phase() {
        let cap = RhythmBlockMetrics::cap(georgia_line(), 11.09, 3, 0);
        let row = cap.first_rows(2);
        let expected = cap.first_baseline() + 2.0 * cap.line().line_height();
        assert!((cap.baseline_at_row(i64::from(row)) - expected).abs() < 1e-5);
        assert!((cap.baseline_at_row(0) - 11.09).abs() < 1e-6);
    }

    #[test]
    fn covering_fits_every_mixture_of_the_set_it_covers() {
        // A body style plus the faces its runs can pull in: a monospace with
        // a deeper descent and a display face with a far taller ascent.
        let body = georgia_line();
        let mono = RhythmLineMetrics::new(15.0, 6.0, 3, GRID);
        let display = RhythmLineMetrics::new(28.8, 4.0, 3, GRID);
        let covering = RhythmLineMetrics::covering(&[body, mono, display], GRID);

        // Maxima, not any single member's pair: the worst case is a line
        // mixing the display ascent with the monospace descent.
        assert_eq!(covering.ascent(), 28.8);
        assert_eq!(covering.descent(), 6.0);
        assert_eq!(covering.line_rhythms(), 5); // 34.8px of ink needs 5 rows
        assert!(!covering.overflows_line_box());

        // The point of the fold: at that count no mixture overflows, so the
        // count is a static row budget settled before anything is shaped.
        for ascent in [body.ascent(), mono.ascent(), display.ascent()] {
            for descent in [body.descent(), mono.descent(), display.descent()] {
                let line = RhythmLineMetrics::new(ascent, descent, covering.line_rhythms(), GRID);
                assert!(!line.overflows_line_box(), "{ascent} / {descent} overflows");
            }
        }
    }

    #[test]
    fn covering_keeps_the_tallest_requested_line_height() {
        // Ink that fits everywhere: the count comes from the members, not
        // from the ink, and a single-member fold is the member itself.
        let short = RhythmLineMetrics::new(14.67, 3.51, 3, GRID);
        let tall = RhythmLineMetrics::new(10.0, 3.0, 5, GRID);
        assert_eq!(
            RhythmLineMetrics::covering(&[short, tall], GRID).line_rhythms(),
            5
        );
        assert_eq!(RhythmLineMetrics::covering(&[short], GRID), short);
    }

    #[test]
    #[should_panic(expected = "must cover at least one line")]
    fn covering_rejects_an_empty_set() {
        let _ = RhythmLineMetrics::covering(&[], GRID);
    }

    #[test]
    #[should_panic(expected = "must be built on the covering grid")]
    fn covering_rejects_a_line_from_another_grid() {
        let other = RhythmLineMetrics::new(14.67, 3.51, 3, Rhythm::new(10.0));
        let _ = RhythmLineMetrics::covering(&[georgia_line(), other], GRID);
    }

    #[test]
    fn negative_anchor_counts_stay_meaningful_as_margins() {
        // A zero `top` puts the line box above the block's own top edge —
        // meaningful as a margin overlap, exactly like `baseline_top`.
        let block = RhythmBlockMetrics::new(georgia_line(), 0, -1);
        assert!(block.line().paint_origin_for(block.first_baseline()) < 0.0);
        assert_eq!(block.rows(2), 0 - 1 + 3);
    }

    #[test]
    #[should_panic(expected = "at least one line")]
    fn fragments_reject_zero_lines() {
        let _ = RhythmBlockMetrics::new(georgia_line(), 3, 1).rows(0);
    }

    #[test]
    #[should_panic(expected = "cap height must be finite and greater than zero")]
    fn cap_blocks_reject_an_unusable_cap_height() {
        let _ = RhythmBlockMetrics::cap(georgia_line(), 0.0, 3, 0);
    }
}
