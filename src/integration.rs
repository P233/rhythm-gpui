//! The gpui integration: metric resolution through `TextSystem`, `Pixels`-typed
//! spacing, drop caps, the `RhythmStyled` extension, and the debug overlay.

use gpui::{
    canvas, fill, point, px, rgba, size, Bounds, Font, Hsla, IntoElement, ParentElement, Pixels,
    Styled, TextSystem,
};

use crate::{FontRhythm, Rhythm};

/// The vertical rhythm grid in gpui units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RhythmGrid {
    core: Rhythm,
}

impl RhythmGrid {
    /// Create a grid with a finite, positive rhythm-unit size.
    ///
    /// # Panics
    ///
    /// Panics when `size` is zero, negative, or non-finite.
    pub fn new(size: Pixels) -> Self {
        Self {
            core: Rhythm::new(size.into()),
        }
    }

    /// Height of one rhythm unit.
    pub fn size(&self) -> Pixels {
        px(self.core.size())
    }

    fn core(&self) -> Rhythm {
        self.core
    }

    fn assert_font(&self, font: &RhythmFont) {
        assert_eq!(
            *self, font.grid,
            "RhythmFont must use the same grid size as the spacing calculation"
        );
    }

    /// Total height of `n` rhythm units (rhythm-sass `rhythm($n)`).
    pub fn height(&self, n: i32) -> Pixels {
        px(self.core().height(n))
    }

    /// Round `height` up to whole rhythm rows — the pad strategy for content
    /// whose height is not rhythm-controlled; see [`Rhythm::snap_up`]. With a
    /// known width this is all a media block needs:
    /// `div().w(w).h(grid.snap_up(w / ratio))`; for fluid widths use
    /// [`rhythm_frame`](crate::rhythm_frame).
    ///
    /// # Panics
    ///
    /// Panics when `height` is negative or non-finite.
    pub fn snap_up(&self, height: Pixels) -> Pixels {
        px(self.core().snap_up(height.into()))
    }

    /// Round `height` down to whole rhythm rows — the crop strategy; see
    /// [`Rhythm::snap_down`].
    ///
    /// # Panics
    ///
    /// Panics when `height` is negative or non-finite.
    pub fn snap_down(&self, height: Pixels) -> Pixels {
        px(self.core().snap_down(height.into()))
    }

    /// Resolve a font bound to this grid — [`RhythmFont::resolve`] with the
    /// grid slot filled in; see it for the fallback-resolution caveats.
    pub fn font(
        &self,
        text_system: &TextSystem,
        font: Font,
        font_size: Pixels,
        line_rhythms: u32,
    ) -> RhythmFont {
        RhythmFont::resolve(text_system, font, font_size, line_rhythms, *self)
    }

    /// Deprecated form of [`RhythmFont::baseline_top`].
    ///
    /// # Panics
    ///
    /// Panics when `font` was resolved against a different grid size.
    #[deprecated(since = "0.2.0", note = "use `RhythmFont::baseline_top`")]
    pub fn baseline_top(&self, font: &RhythmFont, n: i32) -> Pixels {
        self.assert_font(font);
        font.baseline_top(n)
    }

    /// Deprecated form of [`RhythmFont::baseline_bottom`].
    ///
    /// # Panics
    ///
    /// Panics when `font` was resolved against a different grid size.
    #[deprecated(since = "0.2.0", note = "use `RhythmFont::baseline_bottom`")]
    pub fn baseline_bottom(&self, font: &RhythmFont, n: i32) -> Pixels {
        self.assert_font(font);
        font.baseline_bottom(n)
    }

    /// Deprecated form of [`RhythmFont::baseline_between`].
    ///
    /// # Panics
    ///
    /// Panics when `above` or `below` was resolved against a different grid size.
    #[deprecated(since = "0.2.0", note = "use `RhythmFont::baseline_between`")]
    pub fn baseline_between(&self, above: &RhythmFont, below: &RhythmFont, n: i32) -> Pixels {
        self.assert_font(above);
        above.baseline_between(below, n)
    }

    /// Deprecated form of [`RhythmFont::cap_top`].
    ///
    /// # Panics
    ///
    /// Panics when `font` was resolved against a different grid size.
    #[deprecated(since = "0.2.0", note = "use `RhythmFont::cap_top`")]
    pub fn cap_top(&self, font: &RhythmFont, n: i32) -> Option<Pixels> {
        self.assert_font(font);
        font.cap_top(n)
    }

    /// Deprecated form of [`RhythmFont::cap_bottom`].
    ///
    /// # Panics
    ///
    /// Panics when `font` was resolved against a different grid size.
    #[deprecated(since = "0.2.0", note = "use `RhythmFont::cap_bottom`")]
    pub fn cap_bottom(&self, font: &RhythmFont, m: i32) -> Option<Pixels> {
        self.assert_font(font);
        font.cap_bottom(m)
    }
}

/// A requested gpui font bound to the rhythm grid, with vertical metrics from
/// the font gpui actually resolved. When the requested family is unavailable,
/// that may be a fallback font; see [`Self::resolve`].
#[derive(Debug, Clone)]
pub struct RhythmFont {
    font: Font,
    metrics: FontRhythm,
    grid: RhythmGrid,
}

impl RhythmFont {
    /// Resolve `font`'s metrics at `font_size` through gpui's text system.
    ///
    /// If gpui cannot load the requested font, its [`TextSystem::resolve_font`]
    /// silently tries the configured fallback stack. The returned value retains
    /// the requested [`Font`] configuration, while its metrics come from the
    /// resolved fallback; applying it through [`RhythmStyled::rhythm_font`]
    /// follows gpui's same resolution policy. Check
    /// [`TextSystem::all_font_names`] before calling this method when using the
    /// exact family is a requirement.
    ///
    /// Metrics come from the resolved primary font only. A shaped line has one
    /// baseline, placed from the tallest run's ascent, so explicitly mixing a
    /// taller font (e.g. CJK) into the same element shifts every run's baseline
    /// together — lay each font out as its own element to keep them on the
    /// grid. Glyph-level fallback is different: substituted glyphs borrow the
    /// primary font's baseline and never enter the line's ascent.
    pub fn resolve(
        text_system: &TextSystem,
        font: Font,
        font_size: Pixels,
        line_rhythms: u32,
        grid: RhythmGrid,
    ) -> Self {
        let font_id = text_system.resolve_font(&font);
        // gpui's FontMetrics keeps the OpenType sign convention where descent is
        // negative below the baseline (its paint path negates it before use);
        // from_platform_metrics normalizes signs and drops unusable cap/x heights.
        let metrics = FontRhythm::from_platform_metrics(
            font_size.into(),
            line_rhythms,
            text_system.ascent(font_id, font_size).into(),
            text_system.descent(font_id, font_size).into(),
            text_system.cap_height(font_id, font_size).into(),
            text_system.x_height(font_id, font_size).into(),
        );

        Self {
            font,
            metrics,
            grid,
        }
    }

    /// Compatibility constructor for a Plumber/rhythm-sass `baseline-ratio`.
    /// Prefer [`Self::resolve`]; see [`FontRhythm::from_baseline_ratio`].
    pub fn from_baseline_ratio(
        font: Font,
        font_size: Pixels,
        line_rhythms: u32,
        baseline_ratio: f32,
        grid: RhythmGrid,
    ) -> Self {
        Self {
            font,
            metrics: FontRhythm::from_baseline_ratio(
                font_size.into(),
                line_rhythms,
                baseline_ratio,
            ),
            grid,
        }
    }

    /// The requested gpui font configuration applied by
    /// [`RhythmStyled::rhythm_font`].
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// The grid this font was resolved against.
    pub const fn grid(&self) -> RhythmGrid {
        self.grid
    }

    /// Top spacing that lands the first baseline `n` rhythm units below the
    /// element's padding edge (rhythm-sass `baseline-top()` / `rhythm-bottom()`).
    ///
    /// Negative when `n × grid size` is smaller than
    /// [`baseline_above`](Self::baseline_above) — meaningful as a margin, not
    /// as a padding.
    pub fn baseline_top(&self, n: i32) -> Pixels {
        px(self.grid.core().baseline_top(&self.metrics, n))
    }

    /// Bottom spacing that puts the nth grid line below the last baseline at
    /// the element's padding edge (rhythm-sass `baseline-bottom()` /
    /// `rhythm-top()`).
    ///
    /// Negative when `n × grid size` is smaller than the baseline-to-bottom
    /// distance — meaningful as a margin, not as a padding.
    pub fn baseline_bottom(&self, n: i32) -> Pixels {
        px(self.grid.core().baseline_bottom(&self.metrics, n))
    }

    /// Spacing from a block set in this font down to a following block set in
    /// `below`, so the two adjacent baselines are exactly `n` rhythm units
    /// apart (rhythm-sass `baseline-between()`).
    ///
    /// gpui's flex layout never collapses margins, so apply the result to
    /// exactly one side (or as a `gap`), unlike the CSS original. Negative
    /// results overlap the blocks when applied.
    ///
    /// # Panics
    ///
    /// Panics when `below` was resolved against a different grid size; its
    /// line height would no longer match the calculated spacing.
    pub fn baseline_between(&self, below: &RhythmFont, n: i32) -> Pixels {
        assert_eq!(
            self.grid, below.grid,
            "both fonts must be resolved against the same grid size"
        );
        px(self
            .grid
            .core()
            .baseline_between(&self.metrics, &below.metrics, n))
    }

    /// Top spacing that lands the capitals' ink top — not the baseline — on
    /// the nth grid line, for optically-aligned openings. Close the block
    /// with [`Self::cap_bottom`], not [`Self::baseline_bottom`]; see
    /// [`Rhythm::cap_top`] for the contract, or use [`Self::cap_span`] to get
    /// the pair in one call. `None` when the font has no usable cap height.
    pub fn cap_top(&self, n: i32) -> Option<Pixels> {
        self.grid.core().cap_top(&self.metrics, n).map(px)
    }

    /// Bottom spacing pairing [`Self::cap_top`], returning the trimmed space
    /// so the block closes on whole rhythm rows; see [`Rhythm::cap_bottom`].
    /// `None` when the font has no usable cap height.
    pub fn cap_bottom(&self, m: i32) -> Option<Pixels> {
        self.grid.core().cap_bottom(&self.metrics, m).map(px)
    }

    /// The cap-anchored opening as one paired value:
    /// `(cap_top(top), cap_bottom(bottom))`. Taking the pair from a single
    /// call makes the off-grid mistake — a cap opening closed with
    /// [`Self::baseline_bottom`] — inexpressible.
    ///
    /// `None` when the font has no usable cap height. The baseline fallback
    /// is a design choice (the equivalent baseline count differs from `top`
    /// by the cap height), so pick it explicitly, e.g. with gpui's `.map()`:
    ///
    /// ```no_run
    /// # use gpui::{div, prelude::*};
    /// # use rhythm_gpui::RhythmFont;
    /// # fn opening(heading: &RhythmFont) -> impl IntoElement {
    /// div().map(|d| match heading.cap_span(4, 0) {
    ///     Some((pt, pb)) => d.pt(pt).pb(pb),
    ///     None => d.pt(heading.baseline_top(7)),
    /// })
    /// # }
    /// ```
    pub fn cap_span(&self, top: i32, bottom: i32) -> Option<(Pixels, Pixels)> {
        Some((self.cap_top(top)?, self.cap_bottom(bottom)?))
    }

    /// Resolve `font` as a drop cap sunk `lines` lines deep into text set in
    /// this font — [`RhythmDropCap::resolve`] with the body slot filled in;
    /// see it for the solving contract.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or `lines × line_rhythms` overflows `u32`.
    pub fn drop_cap(&self, text_system: &TextSystem, font: Font, lines: u32) -> RhythmDropCap {
        RhythmDropCap::resolve(text_system, font, self, lines)
    }

    /// Resolved vertical metrics in logical pixels.
    ///
    /// These belong to the fallback font when gpui could not load the requested
    /// family; see [`Self::resolve`].
    pub const fn metrics(&self) -> &FontRhythm {
        &self.metrics
    }

    /// The font size the metrics were resolved at.
    pub fn font_size(&self) -> Pixels {
        px(self.metrics.font_size())
    }

    /// The rhythm line height: `line_rhythms × grid size`.
    pub fn line_height(&self) -> Pixels {
        px(self.metrics.line_height(self.grid.core()))
    }

    /// Distance from the top of the line box down to the baseline, as gpui will
    /// paint it. Useful for custom elements and debug overlays.
    pub fn baseline_above(&self) -> Pixels {
        px(self.metrics.baseline_above(self.grid.core()))
    }

    /// Invisible space above the cap height; subtract from a top spacing (or apply
    /// as a negative margin) for CSS `text-box-trim`-style optical alignment.
    /// `None` when the metrics source has no usable cap height, including values
    /// created with [`Self::from_baseline_ratio`].
    pub fn cap_trim_top(&self) -> Option<Pixels> {
        self.metrics.cap_trim_top(self.grid.core()).map(px)
    }

    /// Like [`Self::cap_trim_top`] but trimming to the x-height. `None` when the
    /// metrics source has no usable x-height.
    pub fn x_trim_top(&self) -> Option<Pixels> {
        self.metrics.x_trim_top(self.grid.core()).map(px)
    }
}

/// A drop cap bound to the grid: the cap face at the size solved by
/// [`FontRhythm::drop_cap`], plus the inset anchoring its baseline. Apply with
/// [`RhythmStyled::rhythm_drop_cap`]; for wrap-around text, measure the letter
/// with `shape_line` (see `drop_cap_paragraph` in the `demo` example).
///
/// # Examples
///
/// ```no_run
/// use gpui::{div, font, prelude::*, px, FontWeight, TextSystem};
/// use rhythm_gpui::{RhythmDropCap, RhythmFont, RhythmGrid, RhythmStyled};
///
/// fn drop_cap_block(text_system: &TextSystem) -> impl IntoElement {
///     let grid = RhythmGrid::new(px(8.));
///     let body = RhythmFont::resolve(text_system, font("Georgia"), px(16.), 3, grid);
///     let mut bold = font("Georgia");
///     bold.weight = FontWeight::BOLD;
///     let cap = RhythmDropCap::resolve(text_system, bold, &body, 3);
///
///     div()
///         .flex()
///         .items_start()
///         .gap(px(12.))
///         .child(div().rhythm_drop_cap(&cap).child("W"))
///         .child(div().flex_1().min_w_0().rhythm_font(&body).child("hen…"))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RhythmDropCap {
    font: RhythmFont,
    top: Pixels,
}

impl RhythmDropCap {
    /// Resolve `font` as a drop cap sunk `lines` lines deep into `body` text.
    ///
    /// The solved size spans the capital from the first line's cap top down to
    /// the `lines`-th baseline. The baseline anchor is exact even when a
    /// missing cap height falls back to the 0.7 em approximation; the fallback
    /// only affects the visual top. See [`FontRhythm::drop_cap`] for the math.
    ///
    /// # Panics
    ///
    /// Panics when `lines` is zero or `lines × body.metrics().line_rhythms()`
    /// overflows `u32`.
    pub fn resolve(text_system: &TextSystem, font: Font, body: &RhythmFont, lines: u32) -> Self {
        // The probe's line box is irrelevant: drop_cap reads only metric ratios.
        let probe = RhythmFont::resolve(text_system, font.clone(), body.font_size(), 1, body.grid);
        let solved = body
            .metrics()
            .drop_cap(probe.metrics(), lines, body.grid.core());
        Self {
            font: RhythmFont {
                font,
                metrics: *solved.metrics(),
                grid: body.grid,
            },
            top: px(solved.top()),
        }
    }

    /// The cap face at the solved size; its line box spans the sunk lines.
    pub const fn font(&self) -> &RhythmFont {
        &self.font
    }

    /// Relative `top` inset landing the cap's baseline on the last sunk line's
    /// baseline. An inset rather than a margin on purpose: cap-heavy faces
    /// (cap height exceeding `ascent − descent`, e.g. Merriweather) need a
    /// downward shift, and a positive margin would grow the flex row's cross
    /// size and push everything below off the grid.
    pub const fn top(&self) -> Pixels {
        self.top
    }
}

/// Extension methods for applying rhythm fonts through gpui's fluent style API.
pub trait RhythmStyled: Styled + Sized {
    /// Apply the complete font configuration, size, and rhythm line height.
    fn rhythm_font(self, font: &RhythmFont) -> Self {
        self.font(font.font().clone())
            .text_size(font.font_size())
            .line_height(font.line_height())
    }

    /// The whole text-block recipe in one call: [`Self::rhythm_font`] plus
    /// the paired paddings that open `top` rhythm units above the first
    /// baseline and close `bottom` units below the last, so the block
    /// occupies a whole number of rhythm rows for any number of wrapped
    /// lines and composes freely without breaking the page rhythm.
    ///
    /// Paddings go negative when `top`/`bottom` are smaller than the font's
    /// baseline distances; negative spacing is meaningful as a margin, so
    /// use [`RhythmFont::baseline_top`] / [`RhythmFont::baseline_bottom`]
    /// directly for margin-based layouts.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gpui::{div, font, prelude::*, px, TextSystem};
    /// use rhythm_gpui::{RhythmGrid, RhythmStyled};
    ///
    /// fn card(text_system: &TextSystem) -> impl IntoElement {
    ///     let grid = RhythmGrid::new(px(8.));
    ///     let body = grid.font(text_system, font("Georgia"), px(16.), 3);
    ///     div()
    ///         .rhythm_block(&body, 3, 1)
    ///         .child("A block spanning whole rhythm rows.")
    /// }
    /// ```
    fn rhythm_block(self, font: &RhythmFont, top: i32, bottom: i32) -> Self {
        self.rhythm_font(font)
            .pt(font.baseline_top(top))
            .pb(font.baseline_bottom(bottom))
    }

    /// Apply a drop cap: the solved font plus its baseline-anchoring relative
    /// `top` inset. See [`RhythmDropCap::top`] for why the anchor must not be
    /// applied as a margin.
    fn rhythm_drop_cap(self, cap: &RhythmDropCap) -> Self {
        self.rhythm_font(cap.font()).relative().top(cap.top())
    }

    /// Paint the debug grid over this element while `show` is true, in the
    /// classic translucent red (`0xff78783f`). Chain it after the content
    /// children so the stripes paint on top; the element's top edge becomes
    /// the grid origin, and the element's own position style is left
    /// untouched. Use [`rhythm_overlay`] directly to pick a color.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gpui::{div, prelude::*, px};
    /// use rhythm_gpui::{RhythmGrid, RhythmStyled};
    ///
    /// fn page(show_grid: bool) -> impl IntoElement {
    ///     let grid = RhythmGrid::new(px(8.));
    ///     div()
    ///         .child("…content on the grid…")
    ///         .rhythm_debug_overlay(grid, show_grid)
    /// }
    /// ```
    fn rhythm_debug_overlay(self, grid: RhythmGrid, show: bool) -> Self
    where
        Self: ParentElement,
    {
        if show {
            self.child(rhythm_overlay(grid, rgba(0xff78783f)))
        } else {
            self
        }
    }
}

impl<T: Styled> RhythmStyled for T {}

/// A `draw-rhythms` debug overlay: paints every other grid row in `color`.
/// Place it as the last child of the container it should cover; it fills that
/// container and ignores mouse events. An ordinary gpui container already uses
/// relative positioning by default, so no extra `.relative()` call is needed,
/// and this helper does not alter the container's position style. Equivalent to
/// the rhythm-sass `draw-rhythms()` mixin.
///
/// If the content scrolls, put the overlay *inside* the scrolled wrapper so
/// the grid moves with the text.
pub fn rhythm_overlay(grid: RhythmGrid, color: impl Into<Hsla>) -> impl IntoElement {
    let color = color.into();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let mut y = bounds.origin.y;
            while y < bounds.bottom() {
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.origin.x, y),
                        size(bounds.size.width, grid.size()),
                    ),
                    color,
                ));
                y += grid.size() * 2.;
            }
        },
    )
    .absolute()
    .inset_0()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        font, AnyElement, FontFallbacks, FontFeatures, FontStyle, Position, StyleRefinement,
    };

    #[derive(Default)]
    struct CapturedStyle {
        style: StyleRefinement,
    }

    impl Styled for CapturedStyle {
        fn style(&mut self) -> &mut StyleRefinement {
            &mut self.style
        }
    }

    #[test]
    fn rhythm_font_applies_the_resolved_font_contract() {
        let mut resolved_font = font("Example Serif");
        resolved_font.features = FontFeatures::disable_ligatures();
        resolved_font.fallbacks = Some(FontFallbacks::from_fonts(vec!["Fallback Serif".into()]));
        resolved_font.style = FontStyle::Oblique;
        let expected = resolved_font.clone();
        let rhythm_font = RhythmFont::from_baseline_ratio(
            resolved_font,
            px(16.0),
            3,
            0.2,
            RhythmGrid::new(px(8.0)),
        );

        let captured = CapturedStyle::default().rhythm_font(&rhythm_font);
        let text = captured
            .style
            .text
            .expect("rhythm font should set text style");
        assert_eq!(text.font_family, Some(expected.family));
        assert_eq!(text.font_features, Some(expected.features));
        assert_eq!(text.font_fallbacks, expected.fallbacks);
        assert_eq!(text.font_weight, Some(expected.weight));
        assert_eq!(text.font_style, Some(expected.style));
    }

    #[derive(Default)]
    struct CapturedChildren {
        style: StyleRefinement,
        children: Vec<AnyElement>,
    }

    impl Styled for CapturedChildren {
        fn style(&mut self) -> &mut StyleRefinement {
            &mut self.style
        }
    }

    impl ParentElement for CapturedChildren {
        fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
            self.children.extend(elements)
        }
    }

    #[test]
    fn rhythm_debug_overlay_appends_only_when_shown() {
        let grid = RhythmGrid::new(px(8.0));
        let hidden = CapturedChildren::default().rhythm_debug_overlay(grid, false);
        assert!(hidden.children.is_empty());
        let shown = CapturedChildren::default().rhythm_debug_overlay(grid, true);
        assert_eq!(shown.children.len(), 1);
    }

    #[test]
    fn rhythm_drop_cap_anchors_with_a_relative_inset_not_a_margin() {
        let grid = RhythmGrid::new(px(8.0));
        let body = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);
        let solved = body.metrics().drop_cap(body.metrics(), 3, Rhythm::new(8.0));
        let cap = RhythmDropCap {
            font: RhythmFont {
                font: font("Example Serif"),
                metrics: *solved.metrics(),
                grid,
            },
            top: px(solved.top()),
        };

        let captured = CapturedStyle::default().rhythm_drop_cap(&cap);
        assert_eq!(captured.style.position, Some(Position::Relative));
        assert_eq!(captured.style.inset.top, Some(cap.top().into()));
        assert_eq!(captured.style.margin.top, None);
        let text = captured.style.text.expect("drop cap should set text style");
        assert_eq!(text.font_family, Some("Example Serif".into()));
    }

    #[test]
    #[should_panic(expected = "same grid size")]
    fn baseline_between_rejects_fonts_on_different_grids() {
        let above = RhythmFont::from_baseline_ratio(
            font("Example Serif"),
            px(16.0),
            3,
            0.2,
            RhythmGrid::new(px(8.0)),
        );
        let below = RhythmFont::from_baseline_ratio(
            font("Example Serif"),
            px(16.0),
            3,
            0.2,
            RhythmGrid::new(px(10.0)),
        );

        above.baseline_between(&below, 3);
    }

    #[test]
    #[allow(deprecated)]
    fn deprecated_grid_spacing_matches_the_font_methods() {
        let grid = RhythmGrid::new(px(8.0));
        let body = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);

        assert_eq!(grid.baseline_top(&body, 3), body.baseline_top(3));
        assert_eq!(grid.baseline_bottom(&body, 1), body.baseline_bottom(1));
        assert_eq!(
            grid.baseline_between(&body, &body, 6),
            body.baseline_between(&body, 6)
        );
    }

    #[test]
    #[allow(deprecated)]
    #[should_panic(expected = "RhythmFont must use the same grid size")]
    fn deprecated_spacing_still_rejects_a_mismatched_grid() {
        let font = RhythmFont::from_baseline_ratio(
            font("Example Serif"),
            px(16.0),
            3,
            0.2,
            RhythmGrid::new(px(8.0)),
        );

        RhythmGrid::new(px(10.0)).baseline_top(&font, 3);
    }

    #[test]
    fn rhythm_block_applies_the_font_and_the_paired_paddings() {
        let grid = RhythmGrid::new(px(8.0));
        let body = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);

        let captured = CapturedStyle::default().rhythm_block(&body, 3, 1);
        assert_eq!(
            captured.style.padding.top,
            Some(body.baseline_top(3).into())
        );
        assert_eq!(
            captured.style.padding.bottom,
            Some(body.baseline_bottom(1).into())
        );
        let text = captured
            .style
            .text
            .expect("rhythm block should set the text style");
        assert_eq!(text.font_family, Some("Example Serif".into()));
    }

    #[test]
    fn cap_span_pairs_the_anchors_and_spans_whole_rows() {
        let grid = RhythmGrid::new(px(8.0));
        // Georgia-like metrics at 16px on a 3-unit line.
        let metrics = FontRhythm::from_platform_metrics(16.0, 3, 14.67, -3.51, 11.09, 7.70);
        let heading = RhythmFont {
            font: font("Example Serif"),
            metrics,
            grid,
        };

        let (pt, pb) = heading.cap_span(3, 0).expect("cap metrics are present");
        assert_eq!(Some(pt), heading.cap_top(3));
        assert_eq!(Some(pb), heading.cap_bottom(0));
        let rows = f32::from(pt + heading.line_height() + pb) / 8.0;
        assert!((rows - rows.round()).abs() < 1e-3);
    }

    #[test]
    fn cap_spacing_returns_none_without_cap_height() {
        let grid = RhythmGrid::new(px(8.0));
        let font = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);

        assert_eq!(font.cap_top(3), None);
        assert_eq!(font.cap_bottom(1), None);
        assert_eq!(font.cap_span(3, 1), None);
    }

    #[test]
    #[should_panic(expected = "rhythm unit size must be finite and greater than zero")]
    fn grid_rejects_a_non_positive_size() {
        let _ = RhythmGrid::new(px(0.0));
    }
}
