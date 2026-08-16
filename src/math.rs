//! Vertical rhythm math, independent of gpui.
//!
//! # Model
//!
//! A renderer (CSS, gpui, and most UI toolkits) vertically centers a font's
//! `ascent + descent` box inside the line box, placing the baseline at:
//!
//! ```text
//! baseline_from_top = (line_height - ascent - descent) / 2 + ascent
//! ```
//!
//! Given a line height that is an integer number of rhythm units, the functions
//! in this module compute the padding/margin that lands every baseline exactly
//! on a rhythm grid line.
//!
//! All quantities are `f32` logical pixels; unit conversion (rems, device
//! scale) belongs to the integration layer.
//!
//! The Plumber/rhythm-sass `baseline-ratio` is recoverable from font metrics:
//! `ratio = (em + descent - ascent) / (2 * em)`. [`FontRhythm::from_baseline_ratio`]
//! is provided for compatibility with existing measured values.

/// The vertical rhythm grid: a stack of rows, each `size` logical pixels tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rhythm {
    size: f32,
}

impl Rhythm {
    /// Create a grid with a finite, positive rhythm-unit size.
    ///
    /// # Panics
    ///
    /// Panics when `size` is zero, negative, or non-finite.
    pub const fn new(size: f32) -> Self {
        assert!(
            size.is_finite() && size > 0.0,
            "rhythm unit size must be finite and greater than zero"
        );
        Self { size }
    }

    /// Height of one rhythm unit in logical pixels.
    #[inline]
    pub const fn size(&self) -> f32 {
        self.size
    }

    /// Axis-neutral length of `n` rhythm units. Use this when the vertical grid
    /// also supplies horizontal indents, gaps, or padding.
    #[inline]
    pub fn spacing(&self, n: i32) -> f32 {
        self.size * n as f32
    }

    /// Total height of `n` rhythm units. Equivalent to rhythm-sass `rhythm($n)`;
    /// apply offsets with plain addition: `grid.height(5) - 1.0`.
    #[inline]
    pub fn height(&self, n: i32) -> f32 {
        self.spacing(n)
    }

    /// Spacing from an element's top edge up to the nth grid line above the first
    /// baseline. Equivalent to rhythm-sass `baseline-top()` / `rhythm-bottom()`.
    ///
    /// Applied as `padding-top` (or `margin-top`), it makes the first baseline land
    /// exactly `n` rhythm units below the grid line at the element's padding edge.
    ///
    /// The result is negative when `n × size` is smaller than the font's
    /// [`baseline_above`](FontRhythm::baseline_above); a negative value is
    /// meaningful as a margin but not as a padding, so pick `n` accordingly.
    #[inline]
    pub fn baseline_top(&self, font: &FontRhythm, n: i32) -> f32 {
        self.height(n) - font.baseline_above(*self)
    }

    /// Spacing from an element's bottom edge down to the nth grid line below the
    /// last baseline. Equivalent to rhythm-sass `baseline-bottom()` / `rhythm-top()`.
    ///
    /// The result is negative when `n × size` is smaller than the font's
    /// [`baseline_below`](FontRhythm::baseline_below); a negative value is
    /// meaningful as a margin but not as a padding, so pick `n` accordingly.
    #[inline]
    pub fn baseline_bottom(&self, font: &FontRhythm, n: i32) -> f32 {
        self.height(n) - font.baseline_below(*self)
    }

    /// Spacing between two adjacent text blocks, measured from the bottom of the
    /// above font's line box to the top of the below font's line box, such that the
    /// two adjacent baselines are exactly `n` rhythm units apart.
    /// Equivalent to rhythm-sass `baseline-between()`.
    ///
    /// The result is negative when `n` rhythm units cannot fit both fonts'
    /// baseline distances; negative values overlap the blocks when applied.
    #[inline]
    pub fn baseline_between(&self, above: &FontRhythm, below: &FontRhythm, n: i32) -> f32 {
        self.baseline_bottom(above, n) - below.baseline_above(*self)
    }

    /// Top spacing that lands the **cap ink top** — not the baseline — on the
    /// nth grid line: `n × size − cap_trim_top`. The grid-woven analog of CSS
    /// `text-box-trim: trim-start` with `text-box-edge: cap`, for openings
    /// where the eye measures ink to edge (heroes, cards, page tops).
    ///
    /// The block's baselines shift off the grid by `cap_height mod size`;
    /// close the block with [`Self::cap_bottom`] — not
    /// [`Self::baseline_bottom`] — so it still occupies a whole number of
    /// rhythm rows and everything after it stays in rhythm.
    ///
    /// `None` when `font` has no usable cap height.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhythm_gpui::{FontRhythm, Rhythm};
    ///
    /// let grid = Rhythm::new(8.0);
    /// // Georgia-like 28px heading on a 5-unit (40px) line.
    /// let heading = FontRhythm::from_platform_metrics(28.0, 5, 25.68, -6.14, 19.40, 13.48);
    ///
    /// // Open: the capitals' ink starts exactly on the 3rd grid line…
    /// let pt = grid.cap_top(&heading, 3).unwrap();
    /// assert!((pt + heading.cap_trim_top(grid).unwrap() - grid.height(3)).abs() < 1e-3);
    ///
    /// // …and the paired closer returns the trim, so the block spans whole rows.
    /// let pb = grid.cap_bottom(&heading, 0).unwrap();
    /// let block = pt + heading.line_height(grid) + pb;
    /// assert!((block - 64.0).abs() < 1e-3); // 8 whole rows: what follows stays in rhythm
    /// ```
    #[inline]
    pub fn cap_top(&self, font: &FontRhythm, n: i32) -> Option<f32> {
        Some(self.height(n) - font.cap_trim_top(*self)?)
    }

    /// Bottom spacing pairing [`Self::cap_top`]: `m × size + cap_trim_top`,
    /// returning at the bottom exactly what `cap_top` trimmed at the top so
    /// the block occupies a whole number of rhythm rows — for any number of
    /// wrapped lines, since lines advance by whole rows. Equivalently, the
    /// bottom edge lands `m + line_rhythms` units below the last line's cap
    /// top.
    ///
    /// `None` when `font` has no usable cap height.
    #[inline]
    pub fn cap_bottom(&self, font: &FontRhythm, m: i32) -> Option<f32> {
        Some(self.height(m) + font.cap_trim_top(*self)?)
    }

    /// The smallest whole number of rhythm rows covering `height` — the pad
    /// strategy for content whose height is not rhythm-controlled (images,
    /// video, embeds): the block grows to the next grid line and the
    /// remainder, always under one unit, becomes trailing whitespace.
    ///
    /// Heights within a few `f32` rounding steps of a whole-row height count
    /// as exact, absorbing float error from measured sizes without allowing a
    /// large rhythm unit to hide a visible remainder. Unlike [`snap`], which
    /// rounds a final value to the nearest step of an arbitrary size, this
    /// rounds heights outward to whole rhythm rows.
    ///
    /// # Panics
    ///
    /// Panics when `height` is negative or non-finite.
    #[inline]
    pub fn snap_up(&self, height: f32) -> f32 {
        self.size * self.snap_rows(height, f32::ceil)
    }

    /// The largest whole number of rhythm rows within `height` — the crop
    /// strategy: the block shrinks to the previous grid line, cutting less
    /// than one unit and never scaling the content up. Same exactness
    /// tolerance as [`Self::snap_up`].
    ///
    /// # Panics
    ///
    /// Panics when `height` is negative or non-finite.
    #[inline]
    pub fn snap_down(&self, height: f32) -> f32 {
        self.size * self.snap_rows(height, f32::floor)
    }

    fn snap_rows(&self, height: f32, round_out: fn(f32) -> f32) -> f32 {
        assert!(
            height.is_finite() && height >= 0.0,
            "height must be finite and non-negative"
        );
        let rows = height / self.size;
        let nearest = rows.round();
        let nearest_height = self.size * nearest;
        // Division and multiplication can each move an exact multiple by a
        // few ULPs. Scale by the measured height, not by the grid size, so the
        // tolerance cannot hide a visible remainder on a large grid.
        let tolerance = height * f32::EPSILON * 8.0;
        if (height - nearest_height).abs() <= tolerance {
            nearest
        } else {
            round_out(rows)
        }
    }
}

/// Vertical metrics of one text style participating in the rhythm grid.
///
/// `ascent` and `descent` are resolved at `font_size` (logical pixels, both
/// positive). Line height is expressed in whole rhythm units, which is what keeps
/// consecutive lines of the same style on the grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontRhythm {
    font_size: f32,
    line_rhythms: u32,
    ascent: f32,
    descent: f32,
    cap_height: Option<f32>,
    x_height: Option<f32>,
}

impl FontRhythm {
    /// Build from real font metrics resolved at `font_size`.
    ///
    /// # Panics
    ///
    /// Panics when `font_size` or `ascent` is not finite and positive, when
    /// `line_rhythms` is zero, or when `descent` is not finite and non-negative.
    pub fn from_metrics(font_size: f32, line_rhythms: u32, ascent: f32, descent: f32) -> Self {
        assert!(
            font_size.is_finite() && font_size > 0.0,
            "font_size must be finite and greater than zero"
        );
        assert!(line_rhythms > 0, "line_rhythms must be greater than zero");
        assert!(
            ascent.is_finite() && ascent > 0.0,
            "ascent must be finite and greater than zero"
        );
        assert!(
            descent.is_finite() && descent >= 0.0,
            "descent must be finite and non-negative"
        );
        Self {
            font_size,
            line_rhythms,
            ascent,
            descent,
            cap_height: None,
            x_height: None,
        }
    }

    /// Build from platform-reported metrics, normalizing conventions.
    ///
    /// Some platforms report table metrics in the OpenType sign convention where
    /// descent is negative below the baseline (gpui on macOS does), so `ascent`
    /// and `descent` are taken by magnitude. Non-finite or non-positive cap and
    /// x heights are treated as unavailable and become `None`.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Self::from_metrics`] after
    /// normalization (e.g. a zero `ascent`).
    pub fn from_platform_metrics(
        font_size: f32,
        line_rhythms: u32,
        ascent: f32,
        descent: f32,
        cap_height: f32,
        x_height: f32,
    ) -> Self {
        let mut font = Self::from_metrics(font_size, line_rhythms, ascent.abs(), descent.abs());
        font.cap_height = usable_metric(cap_height);
        font.x_height = usable_metric(x_height);
        font
    }

    /// Compatibility constructor for a Plumber/rhythm-sass `baseline-ratio`
    /// (`0 < ratio < 1`), using the em-box approximation the Sass library assumed:
    /// `ascent = (1 - ratio) * font_size`, `descent = ratio * font_size`.
    ///
    /// Prefer [`Self::from_metrics`]: the ratio is itself derived from metrics
    /// (`(em + descent - ascent) / (2 * em)`) and the em-box model slightly
    /// misplaces the baseline for fonts whose `ascent + descent != em`.
    ///
    /// # Panics
    ///
    /// Panics when `baseline_ratio` is not strictly between 0 and 1, when
    /// `font_size` is not finite and positive, or when `line_rhythms` is zero.
    pub fn from_baseline_ratio(font_size: f32, line_rhythms: u32, baseline_ratio: f32) -> Self {
        assert!(
            baseline_ratio > 0.0 && baseline_ratio < 1.0,
            "baseline ratio must be strictly between 0 and 1"
        );
        Self::from_metrics(
            font_size,
            line_rhythms,
            font_size * (1.0 - baseline_ratio),
            font_size * baseline_ratio,
        )
    }

    /// Font size in logical pixels.
    #[inline]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Line height in whole rhythm units.
    #[inline]
    pub const fn line_rhythms(&self) -> u32 {
        self.line_rhythms
    }

    /// Distance from the baseline up to the top of the `ascent` box, positive.
    #[inline]
    pub const fn ascent(&self) -> f32 {
        self.ascent
    }

    /// Distance from the baseline down to the bottom of the `descent` box, positive.
    #[inline]
    pub const fn descent(&self) -> f32 {
        self.descent
    }

    /// Height of capital letters above the baseline, if known.
    #[inline]
    pub const fn cap_height(&self) -> Option<f32> {
        self.cap_height
    }

    /// Height of a lowercase x above the baseline, if known.
    #[inline]
    pub const fn x_height(&self) -> Option<f32> {
        self.x_height
    }

    /// Set the capital height above the baseline, enabling [`Self::cap_trim_top`].
    /// Non-finite or non-positive values are treated as unavailable.
    pub fn with_cap_height(mut self, cap_height: f32) -> Self {
        self.cap_height = usable_metric(cap_height);
        self
    }

    /// Set the x-height above the baseline, enabling [`Self::x_trim_top`].
    /// Non-finite or non-positive values are treated as unavailable.
    pub fn with_x_height(mut self, x_height: f32) -> Self {
        self.x_height = usable_metric(x_height);
        self
    }

    /// The Plumber-style baseline ratio implied by these metrics.
    #[inline]
    pub fn baseline_ratio(&self) -> f32 {
        (self.font_size + self.descent - self.ascent) / (2.0 * self.font_size)
    }

    /// The line height on `grid`: [`line_rhythms`](Self::line_rhythms) whole rhythm units.
    #[inline]
    pub fn line_height(&self, grid: Rhythm) -> f32 {
        self.line_metrics(grid).line_height()
    }

    /// Extra space split above and below the `ascent + descent` box.
    #[inline]
    pub fn half_leading(&self, grid: Rhythm) -> f32 {
        self.line_metrics(grid).half_leading()
    }

    /// Distance from the top of the line box down to the baseline.
    #[inline]
    pub fn baseline_above(&self, grid: Rhythm) -> f32 {
        self.line_metrics(grid).baseline_above()
    }

    /// Distance from the baseline down to the bottom of the line box.
    #[inline]
    pub fn baseline_below(&self, grid: Rhythm) -> f32 {
        self.line_metrics(grid).baseline_below()
    }

    /// Invisible space between the top of the line box and the cap top: the amount
    /// a leading-trim (CSS `text-box-trim`) would remove. Subtract it from a top
    /// spacing (or apply as negative margin) to visually butt capitals against an
    /// edge or grid line.
    #[inline]
    pub fn cap_trim_top(&self, grid: Rhythm) -> Option<f32> {
        Some(self.baseline_above(grid) - self.cap_height?)
    }

    /// Like [`Self::cap_trim_top`] but trimming to the x-height.
    #[inline]
    pub fn x_trim_top(&self, grid: Rhythm) -> Option<f32> {
        Some(self.baseline_above(grid) - self.x_height?)
    }

    /// This style's line placement on `grid` as a
    /// [`RhythmLineMetrics`](crate::RhythmLineMetrics) — the same value a
    /// shaped line produces, so a custom renderer can place single-style
    /// text (and empty lines) through one code path.
    #[inline]
    pub fn line_metrics(&self, grid: Rhythm) -> crate::RhythmLineMetrics {
        crate::RhythmLineMetrics::new(self.ascent, self.descent, self.line_rhythms, grid)
    }

    /// Solve a drop cap sunk `lines` lines deep into text set in `self`.
    ///
    /// `cap` carries the cap face's metrics resolved at any font size (metrics
    /// scale linearly, so the probe size is irrelevant); its `line_rhythms` is
    /// ignored. The solved font size makes the cap face's capital span from the
    /// first line's cap top down to the `lines`-th baseline, and
    /// [`DropCapRhythm::top`] anchors the baseline there. A face without a
    /// usable cap height falls back to the classic 0.7 em approximation, which
    /// can only misplace the visual top — the baseline anchor stays exact.
    ///
    /// # Examples
    ///
    /// ```
    /// use rhythm_gpui::{FontRhythm, Rhythm};
    ///
    /// let grid = Rhythm::new(8.0);
    /// // Georgia-like metrics at 16px on a 3-unit (24px) line.
    /// let body = FontRhythm::from_platform_metrics(16.0, 3, 14.67, -3.51, 11.09, 7.70);
    /// let cap = body.drop_cap(&body, 3, grid);
    ///
    /// // The capital spans two body lines plus the body cap height…
    /// let span = 2.0 * body.line_height(grid) + body.cap_height().unwrap();
    /// assert!((cap.metrics().cap_height().unwrap() - span).abs() < 1e-3);
    /// // …and `top` lands its baseline exactly on the third body baseline.
    /// let target = body.baseline_above(grid) + 2.0 * body.line_height(grid);
    /// assert!((cap.top() + cap.metrics().baseline_above(grid) - target).abs() < 1e-3);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or `lines × line_rhythms` overflows `u32`.
    pub fn drop_cap(&self, cap: &FontRhythm, lines: u32, grid: Rhythm) -> DropCapRhythm {
        assert!(lines > 0, "a drop cap must sink at least one line");
        let line_rhythms = self
            .line_rhythms
            .checked_mul(lines)
            .expect("drop cap line rhythms overflow u32");
        let body_cap = self.cap_height.unwrap_or(0.7 * self.font_size);
        let cap_height = cap.cap_height;
        let cap_ratio = cap_height.unwrap_or(0.7 * cap.font_size) / cap.font_size;
        let span = (lines - 1) as f32 * self.line_height(grid) + body_cap;
        let font_size = span / cap_ratio;
        let scale = font_size / cap.font_size;
        let mut metrics = FontRhythm::from_metrics(
            font_size,
            line_rhythms,
            cap.ascent * scale,
            cap.descent * scale,
        );
        metrics.cap_height = cap_height.and_then(|h| usable_metric(h * scale));
        metrics.x_height = cap.x_height.and_then(|h| usable_metric(h * scale));
        let target = self.baseline_above(grid) + (lines - 1) as f32 * self.line_height(grid);
        DropCapRhythm {
            top: target - metrics.baseline_above(grid),
            metrics,
        }
    }
}

/// A drop cap solved by [`FontRhythm::drop_cap`]: the cap face's metrics at the
/// solved size plus the offset anchoring its baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropCapRhythm {
    metrics: FontRhythm,
    top: f32,
}

impl DropCapRhythm {
    /// Cap-face metrics at the solved size. The line box spans the sunk lines
    /// exactly: `line_rhythms` is `lines ×` the body's `line_rhythms`.
    #[inline]
    pub const fn metrics(&self) -> &FontRhythm {
        &self.metrics
    }

    /// Offset from the paragraph's top edge down to the cap's line box top,
    /// negative when the cap must sit above it. Apply it visually — in a flex
    /// row as a relative inset, not a margin: cap-heavy faces (cap height
    /// exceeding `ascent − descent`, e.g. Merriweather) yield a positive
    /// offset, and as a margin it would grow the row's cross size and push
    /// everything below off the grid.
    #[inline]
    pub const fn top(&self) -> f32 {
        self.top
    }
}

fn usable_metric(height: f32) -> Option<f32> {
    (height.is_finite() && height > 0.0).then_some(height)
}

/// Round `value` to the nearest multiple of `step` (e.g. `1.0 / scale_factor` to
/// snap a spacing to whole device pixels). The core functions never round, so
/// baselines land on the grid exactly; snap only final applied values.
///
/// # Panics
///
/// Panics when `step` is zero, negative, or non-finite.
pub fn snap(value: f32, step: f32) -> f32 {
    assert!(
        step.is_finite() && step > 0.0,
        "snap step must be finite and greater than zero"
    );
    (value / step).round() * step
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID: Rhythm = Rhythm::new(8.0);
    const NOTO_SERIF_RATIO: f32 = 0.112061;

    // Mirrors __tests__/_font-maps.scss from rhythm-sass.
    fn font_map_3() -> FontRhythm {
        FontRhythm::from_baseline_ratio(17.0, 3, NOTO_SERIF_RATIO)
    }

    fn font_map_5() -> FontRhythm {
        FontRhythm::from_baseline_ratio(12.0, 2, NOTO_SERIF_RATIO)
    }

    #[test]
    fn rhythm_spacing_and_height() {
        assert_eq!(GRID.spacing(5), 40.0);
        assert_eq!(GRID.spacing(0), 0.0);
        assert_eq!(GRID.spacing(-1), -8.0);
        assert_eq!(GRID.height(5), 40.0);
        assert_eq!(GRID.height(5), GRID.spacing(5));
        assert_eq!(GRID.height(5) - 1.0, 39.0); // offsets are plain addition
        assert_eq!(GRID.height(0), 0.0);
        assert_eq!(GRID.height(-1), -8.0);
    }

    // rhythm-sass rounded baseline offsets to whole CSS px before subtracting
    // (see calc-baseline-offset in _lib.scss). The math here stays exact, so the
    // Sass parity tests apply the same rounding explicitly.
    fn sass_baseline_bottom(font: &FontRhythm, n: i32) -> f32 {
        GRID.height(n) - font.baseline_below(GRID).round()
    }

    fn sass_baseline_top(font: &FontRhythm, n: i32) -> f32 {
        GRID.height(n) - font.baseline_above(GRID).round()
    }

    #[test]
    fn sass_parity_rhythm_top_and_bottom() {
        // Expected values from __tests__/rhythm.test.scss.
        assert_eq!(sass_baseline_bottom(&font_map_3(), 3), 19.0); // rhythm-top
        assert_eq!(sass_baseline_bottom(&font_map_3(), 0), -5.0);
        assert_eq!(sass_baseline_bottom(&font_map_3(), -1), -13.0);
        assert_eq!(sass_baseline_top(&font_map_3(), 3), 5.0); // rhythm-bottom
        assert_eq!(sass_baseline_top(&font_map_3(), 0), -19.0);
        assert_eq!(sass_baseline_top(&font_map_3(), -1), -27.0);
    }

    #[test]
    fn sass_parity_baseline_between() {
        let above = font_map_3();
        let below = font_map_5();
        let result = sass_baseline_bottom(&above, 3) - below.baseline_above(GRID).round();
        assert_eq!(result, 6.0);
    }

    #[test]
    fn exact_functions_land_baseline_on_grid() {
        let font = font_map_3();
        // padding_top + renderer's baseline placement = exact grid multiple
        let padding_top = GRID.baseline_top(&font, 3);
        assert!((padding_top + font.baseline_above(GRID) - GRID.height(3)).abs() < 1e-4);

        let padding_bottom = GRID.baseline_bottom(&font, 3);
        assert!((padding_bottom + font.baseline_below(GRID) - GRID.height(3)).abs() < 1e-4);

        // Two stacked blocks: distance between adjacent baselines is n grid units.
        let below = font_map_5();
        let gap = GRID.baseline_between(&font, &below, 3);
        let baseline_distance = font.baseline_below(GRID) + gap + below.baseline_above(GRID);
        assert!((baseline_distance - GRID.height(3)).abs() < 1e-4);
    }

    #[test]
    fn baseline_ratio_roundtrip() {
        let font = font_map_3();
        assert!((font.baseline_ratio() - NOTO_SERIF_RATIO).abs() < 1e-6);
    }

    #[test]
    fn baseline_ratio_from_real_metrics_matches_plumber() {
        // Noto Serif: units_per_em 1000, hhea ascent 1069, descent 293.
        // Plumber's published ratio for Noto Serif is 0.112061.
        let em = 1000.0;
        let font = FontRhythm::from_metrics(em, 1, 1069.0, 293.0);
        assert!((font.baseline_ratio() - 0.112).abs() < 1e-3);
    }

    #[test]
    fn platform_metrics_normalize_opentype_signs() {
        // gpui's FontMetrics reports table descent as negative on macOS
        // (OpenType sign convention); magnitudes must come out positive.
        let font = FontRhythm::from_platform_metrics(16.0, 3, 14.75, -3.25, 11.2, 8.1);
        assert_eq!(font.ascent(), 14.75);
        assert_eq!(font.descent(), 3.25);
        assert_eq!(font.cap_height(), Some(11.2));
        assert_eq!(font.x_height(), Some(8.1));
    }

    #[test]
    fn platform_metrics_filter_unusable_optional_heights() {
        let font = FontRhythm::from_platform_metrics(16.0, 3, 14.75, 3.25, 0.0, f32::NAN);
        assert_eq!(font.cap_height(), None);
        assert_eq!(font.x_height(), None);

        let font = FontRhythm::from_platform_metrics(16.0, 3, 14.75, 3.25, -1.0, 7.5);
        assert_eq!(font.cap_height(), None);
        assert_eq!(font.x_height(), Some(7.5));
    }

    #[test]
    #[should_panic(expected = "line_rhythms must be greater than zero")]
    fn zero_line_rhythms_are_rejected() {
        let _ = FontRhythm::from_metrics(16.0, 0, 14.75, 3.25);
    }

    #[test]
    #[should_panic(expected = "baseline ratio must be strictly between 0 and 1")]
    fn out_of_range_baseline_ratio_is_rejected() {
        let _ = FontRhythm::from_baseline_ratio(16.0, 3, 1.5);
    }

    #[test]
    fn cap_trim() {
        let font = font_map_3().with_cap_height(0.7 * 17.0);
        let trim = font.cap_trim_top(GRID).unwrap();
        assert!((trim - (font.baseline_above(GRID) - 11.9)).abs() < 1e-4);
        assert_eq!(font_map_3().cap_trim_top(GRID), None);
    }

    #[test]
    fn snap_to_device_pixels() {
        assert_eq!(snap(5.4, 0.5), 5.5); // 2x display
        assert_eq!(snap(5.4, 1.0), 5.0);
        assert_eq!(snap(-5.4, 0.5), -5.5);
    }

    #[test]
    fn snap_heights_to_whole_rows() {
        assert_eq!(GRID.snap_up(450.0), 456.0); // 800px wide at 16:9
        assert_eq!(GRID.snap_down(450.0), 448.0);
        assert_eq!(GRID.snap_up(0.0), 0.0);
        assert_eq!(GRID.snap_down(7.9), 0.0); // under one row floors to zero
                                              // Exact multiples pass through both directions.
        assert_eq!(GRID.snap_up(448.0), 448.0);
        assert_eq!(GRID.snap_down(448.0), 448.0);
    }

    #[test]
    fn snap_heights_absorb_float_error_near_whole_rows() {
        assert_eq!(GRID.snap_up(448.0002), 448.0);
        assert_eq!(GRID.snap_down(447.9998), 448.0);
    }

    #[test]
    fn snap_tolerance_does_not_scale_with_the_grid_size() {
        let large_grid = Rhythm::new(1_000_000.0);
        assert_eq!(large_grid.snap_up(50.0), 1_000_000.0);
        assert_eq!(large_grid.snap_down(50.0), 0.0);
    }

    #[test]
    #[should_panic(expected = "height must be finite and non-negative")]
    fn snap_heights_reject_negative_values() {
        let _ = GRID.snap_up(-1.0);
    }

    // Georgia on macOS: upem 2048, hhea 1878/-449, cap 1419, x 986.
    fn georgia_16() -> FontRhythm {
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
    fn drop_cap_baseline_lands_on_the_sunk_line() {
        let body = georgia_16();
        let cap = body.drop_cap(&body, 3, GRID);
        assert_eq!(cap.metrics().line_rhythms(), 9);
        // The capital spans two body lines plus the body cap height…
        let span = 2.0 * body.line_height(GRID) + body.cap_height().unwrap();
        assert!((cap.metrics().cap_height().unwrap() - span).abs() < 1e-3);
        // …and its baseline lands exactly on the third body baseline.
        let target = body.baseline_above(GRID) + 2.0 * body.line_height(GRID);
        assert!((cap.top() + cap.metrics().baseline_above(GRID) - target).abs() < 1e-3);
    }

    #[test]
    fn drop_cap_sizes_by_the_cap_faces_own_metrics() {
        let body = georgia_16();
        // A display face with taller capitals: cap height 0.8 em.
        let display = FontRhythm::from_platform_metrics(12.0, 1, 11.0, -3.0, 9.6, 6.0);
        let cap = body.drop_cap(&display, 3, GRID);
        let span = 2.0 * body.line_height(GRID) + body.cap_height().unwrap();
        // Solved with the cap face's own ratio, not the body's.
        assert!((cap.metrics().font_size() - span / 0.8).abs() < 1e-3);
        assert!((cap.metrics().cap_height().unwrap() - span).abs() < 1e-3);
    }

    #[test]
    fn cap_heavy_faces_need_a_positive_top_offset() {
        // Merriweather: upem 2000, hhea 1968/-546, cap 1486. Cap height
        // (0.743 em) exceeds ascent − descent (0.711 em), which flips the
        // anchor offset positive — the reason the offset must be applied as a
        // relative inset, not a margin (a positive margin grows the flex row's
        // cross size and pushes everything below off the grid).
        let body = FontRhythm::from_platform_metrics(
            16.0,
            3,
            16.0 * 1968.0 / 2000.0,
            -16.0 * 546.0 / 2000.0,
            16.0 * 1486.0 / 2000.0,
            16.0 * 1111.0 / 2000.0,
        );
        let cap = body.drop_cap(&body, 3, GRID);
        assert!(
            cap.top() > 1.0 && cap.top() < 1.1,
            "expected ≈ +1.03, got {}",
            cap.top()
        );
        let target = body.baseline_above(GRID) + 2.0 * body.line_height(GRID);
        assert!((cap.top() + cap.metrics().baseline_above(GRID) - target).abs() < 1e-3);
    }

    #[test]
    fn drop_cap_falls_back_to_the_em_approximation() {
        let body = font_map_3(); // baseline-ratio construction: no cap height
        let cap = body.drop_cap(&body, 2, GRID);
        let expected_size = (body.line_height(GRID) + 0.7 * body.font_size()) / 0.7;
        assert!((cap.metrics().font_size() - expected_size).abs() < 1e-3);
        // The fallback is not fabricated into the solved metrics.
        assert_eq!(cap.metrics().cap_height(), None);
        // The baseline anchor holds regardless.
        let target = body.baseline_above(GRID) + body.line_height(GRID);
        assert!((cap.top() + cap.metrics().baseline_above(GRID) - target).abs() < 1e-3);
    }

    #[test]
    #[should_panic(expected = "at least one line")]
    fn drop_cap_rejects_zero_lines() {
        let _ = font_map_3().drop_cap(&font_map_3(), 0, GRID);
    }

    #[test]
    fn cap_pair_opens_on_ink_and_closes_on_whole_rows() {
        // Georgia-like 28px heading on a 5-unit line.
        let heading = FontRhythm::from_platform_metrics(
            28.0,
            5,
            28.0 * 1878.0 / 2048.0,
            -28.0 * 449.0 / 2048.0,
            28.0 * 1419.0 / 2048.0,
            28.0 * 986.0 / 2048.0,
        );
        let pt = GRID.cap_top(&heading, 3).unwrap();
        // The capitals' ink starts exactly on the 3rd grid line…
        assert!((pt + heading.cap_trim_top(GRID).unwrap() - GRID.height(3)).abs() < 1e-3);
        // …and the paired closer makes the block a whole number of rows.
        let pb = GRID.cap_bottom(&heading, 0).unwrap();
        let block = pt + heading.line_height(GRID) + pb;
        assert!((block - 64.0).abs() < 1e-3);
        // Closing with baseline_bottom instead would leave the fractional
        // cap-height phase inside the block and push everything below it
        // off the grid.
        let mixed = pt + heading.line_height(GRID) + GRID.baseline_bottom(&heading, 1);
        let rows = mixed / GRID.size();
        assert!((rows - rows.round()).abs() > 0.01);
    }

    #[test]
    fn cap_anchors_need_a_usable_cap_height() {
        let no_cap = font_map_3(); // baseline-ratio construction: no cap height
        assert_eq!(GRID.cap_top(&no_cap, 3), None);
        assert_eq!(GRID.cap_bottom(&no_cap, 1), None);
    }

    #[test]
    fn optional_height_setters_filter_unusable_values() {
        let from_setters = font_map_3().with_cap_height(f32::NAN).with_x_height(0.0);
        assert_eq!(from_setters.cap_height(), None);
        assert_eq!(from_setters.x_height(), None);
    }
}
