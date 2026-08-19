//! The gpui integration: metric resolution through `TextSystem`, `Pixels`-typed
//! spacing, drop caps, the `RhythmStyled` extension, and the debug overlay.

use gpui::{
    canvas, fill, point, px, rgba, size, App, Bounds, Font, FontId, Hsla, IntoElement,
    ParentElement, Pixels, RenderOnce, Styled, TextSystem, Window,
};

use crate::{FontRhythm, Rhythm, RhythmBlockMetrics, RhythmLineMetrics};

const DEFAULT_RHYTHM_OVERLAY_RGBA: u32 = 0xff78783f;

fn assert_valid_font_request(font: &Font, font_size: Pixels, line_rhythms: u32) {
    assert!(
        font.weight.0.is_finite() && font.weight.0 > 0.0,
        "font weight must be finite and greater than zero"
    );
    let font_size: f32 = font_size.into();
    assert!(
        font_size.is_finite() && font_size > 0.0,
        "font_size must be finite and greater than zero"
    );
    assert!(line_rhythms > 0, "line_rhythms must be greater than zero");
}

/// Extract the above-baseline edge from a full-frame ideographic glyph.
/// gpui's platform backends report glyph-space bounds with the origin at the
/// ink bottom and positive height toward its top, so requiring the ink to
/// straddle the alphabetic baseline rejects both unsuitable probes and gpui
/// 0.2.2's Linux advance-only placeholder bounds.
fn ideographic_ink_ascent(bounds: Bounds<Pixels>) -> Option<f32> {
    let bottom = f32::from(bounds.origin.y);
    let top = f32::from(bounds.origin.y + bounds.size.height);
    (bottom.is_finite() && bottom < 0.0 && top.is_finite() && top > 0.0).then_some(top)
}

fn select_icf_ascent<E>(
    bounds: impl IntoIterator<Item = Result<Bounds<Pixels>, E>>,
) -> Result<f32, IcfMeasurementError> {
    let mut saw_bounds = false;
    let mut ascent: Option<f32> = None;

    for bounds in bounds {
        let Ok(bounds) = bounds else {
            continue;
        };
        saw_bounds = true;

        if let Some(candidate) = ideographic_ink_ascent(bounds) {
            ascent = Some(ascent.map_or(candidate, |current| current.max(candidate)));
        }
    }

    match ascent {
        Some(ascent) => Ok(ascent),
        None if saw_bounds => Err(IcfMeasurementError::NoUsableBounds),
        None => Err(IcfMeasurementError::NoProbeBounds),
    }
}

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
    #[inline]
    pub fn size(&self) -> Pixels {
        px(self.core.size())
    }

    /// This grid as the dependency-free [`Rhythm`] — the entry to the math
    /// layer, mirroring [`RhythmFont::metrics`] for fonts. Use it to hand this
    /// grid to the `f32` layer: [`FontRhythm`] geometry takes it per call, and
    /// [`RhythmLineMetrics`] takes it once at construction.
    #[inline]
    pub const fn rhythm(&self) -> Rhythm {
        self.core
    }

    /// Axis-neutral length of `n` rhythm units. Use this for horizontal
    /// indents, gaps, or padding measured with the same scale as the vertical
    /// rhythm.
    #[inline]
    pub fn spacing(&self, n: i32) -> Pixels {
        px(self.rhythm().spacing(n))
    }

    /// Total height of `n` rhythm units (rhythm-sass `rhythm($n)`). An exact
    /// alias for [`spacing`](Self::spacing), kept as the vertical name.
    #[inline]
    pub fn height(&self, n: i32) -> Pixels {
        self.spacing(n)
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
    #[inline]
    pub fn snap_up(&self, height: Pixels) -> Pixels {
        px(self.rhythm().snap_up(height.into()))
    }

    /// Round `height` down to whole rhythm rows — the crop strategy; see
    /// [`Rhythm::snap_down`].
    ///
    /// # Panics
    ///
    /// Panics when `height` is negative or non-finite.
    #[inline]
    pub fn snap_down(&self, height: Pixels) -> Pixels {
        px(self.rhythm().snap_down(height.into()))
    }

    /// Metrics for one shaped line on this grid — the `Pixels`-typed entry to
    /// [`RhythmLineMetrics`]. Feed a `WrappedLine`'s `ascent()` / `descent()`
    /// (the shaped maxima over the line's explicit font runs) and the line
    /// height in whole rhythm units; see [`RhythmLineMetrics`] for the
    /// placement contract.
    pub fn line_metrics(
        &self,
        ascent: Pixels,
        descent: Pixels,
        line_rhythms: u32,
    ) -> RhythmLineMetrics {
        RhythmLineMetrics::new(ascent.into(), descent.into(), line_rhythms, self.rhythm())
    }

    /// [`line_metrics`](Self::line_metrics) with `line_rhythms` as a floor:
    /// grow the line box when the shaped ink needs more rows.
    pub fn line_metrics_at_least(
        &self,
        ascent: Pixels,
        descent: Pixels,
        line_rhythms: u32,
    ) -> RhythmLineMetrics {
        RhythmLineMetrics::at_least(ascent.into(), descent.into(), line_rhythms, self.rhythm())
    }

    /// The smallest line box on this grid containing every line in `metrics`.
    ///
    /// # Panics
    ///
    /// Panics when `metrics` is empty or an entry was built on another grid.
    pub fn line_metrics_covering(&self, metrics: &[RhythmLineMetrics]) -> RhythmLineMetrics {
        RhythmLineMetrics::covering(metrics, self.rhythm())
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

    /// A debug overlay on this grid in a custom `color` — [`rhythm_overlay`]
    /// as a grid factory. Pass the result to
    /// [`RhythmStyled::rhythm_debug_overlay`] in place of the bare grid to
    /// customize the stripes without giving up the chainable toggle.
    pub fn overlay(&self, color: impl Into<Hsla>) -> RhythmOverlay {
        rhythm_overlay(*self, color)
    }
}

/// `Pixels`-typed mirrors of the two line values that stay inside a paint
/// path's `Pixels` chain: both reach `WrappedLine::paint`.
///
/// Only four values across this type and [`RhythmBlockMetrics`] are mirrored.
/// The rest of the `f32` surface is read once and converted once, so it is not
/// mirrored: `px(line.ascent())` at the call site is one conversion, while a
/// mirror per accessor doubles the surface to save it. Row counts are never
/// mirrored because they identify grid rows rather than pixel lengths.
impl RhythmLineMetrics {
    /// [`line_height`](Self::line_height) in `Pixels` — the value
    /// `WrappedLine::paint` takes.
    #[inline]
    pub fn line_height_px(&self) -> Pixels {
        px(self.line_height())
    }

    /// [`paint_origin_for`](Self::paint_origin_for) in `Pixels` — the
    /// `origin.y` for `WrappedLine::paint`, from a `Pixels` target baseline.
    #[inline]
    pub fn paint_origin_for_px(&self, target_baseline: Pixels) -> Pixels {
        px(self.paint_origin_for(target_baseline.into()))
    }
}

/// `Pixels`-typed mirrors of the two block values that stay inside a paint
/// path's `Pixels` chain: both are summed with grid lengths into the target
/// baseline [`RhythmLineMetrics::paint_origin_for_px`] consumes. See those
/// mirrors for the integration-boundary rule.
impl RhythmBlockMetrics {
    /// [`first_baseline`](Self::first_baseline) in `Pixels`.
    #[inline]
    pub fn first_baseline_px(&self) -> Pixels {
        px(self.first_baseline())
    }

    /// [`baseline_at_row`](Self::baseline_at_row) in `Pixels`.
    #[inline]
    pub fn baseline_at_row_px(&self, row: i64) -> Pixels {
        px(self.baseline_at_row(row))
    }
}

/// A requested gpui font bound to the rhythm grid, with vertical metrics from
/// the font gpui actually resolved. When the requested family is unavailable,
/// that may be a fallback font; see [`Self::resolve`].
///
/// # Lifecycle
///
/// A `RhythmFont` is an immutable resolved value. [`Self::resolve`] touches
/// gpui's `TextSystem`; optional ideographic-character-face measurement reads
/// it separately through [`Self::measure_icf`] and returns a
/// [`RhythmIcfAnchor`] without changing this value. Metric and spacing methods
/// are pure geometry on the stored values, allocation-free and lock-free,
/// while style application reuses the stored [`Font`] without querying the
/// text system. Register each font family
/// (`TextSystem::add_fonts`) *before its first resolution*: gpui caches failed
/// lookups by [`Font`], so adding a family later and clearing a caller-owned
/// cache does not repair that miss in the same `TextSystem`. Resolution
/// silently falls back otherwise. Re-resolve affected values when typography
/// settings produce a new request key; nothing revalidates an existing value
/// against the text system. The crate keeps no font cache of its own: gpui
/// already caches `Font → FontId` and metrics, so a caller wanting to reuse
/// resolved values owns that map, keyed by [`RhythmFontSpec`], along with its
/// invalidation.
#[derive(Debug, Clone)]
pub struct RhythmFont {
    font: Font,
    font_id: Option<FontId>,
    metrics: FontRhythm,
    grid: RhythmGrid,
}

/// Why an ideographic character-face measurement could not produce an anchor.
///
/// The variants describe what was observable through gpui's public API, not a
/// guessed platform cause. A backend may still substitute a missing glyph —
/// DirectWrite returns `.notdef` in gpui 0.2.2 — so successful measurement also
/// depends on supplying probes covered by the resolved face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IcfMeasurementError {
    /// The [`RhythmFont`] was synthesized without a gpui [`TextSystem`] and
    /// therefore has no resolved [`FontId`] to measure.
    UnresolvedFont,
    /// The probe string was empty.
    EmptyProbes,
    /// gpui returned an error instead of bounds for every probe.
    ///
    /// On gpui 0.2.2's CoreText and Linux backends, this is the result when the
    /// resolved face covers none of the supplied characters. Other backend
    /// errors can produce the same observable result.
    NoProbeBounds,
    /// At least one probe returned bounds, but none were finite and spanned the
    /// alphabetic baseline.
    ///
    /// This rejects both unsuitable glyphs and gpui 0.2.2's Linux
    /// advance-only placeholder bounds.
    NoUsableBounds,
}

impl std::fmt::Display for IcfMeasurementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnresolvedFont => "the rhythm font has no TextSystem-resolved font identity",
            Self::EmptyProbes => "no ICF probe glyphs were supplied",
            Self::NoProbeBounds => "the text backend returned no bounds for any ICF probe",
            Self::NoUsableBounds => {
                "probe bounds were returned, but none described usable glyph ink"
            }
        })
    }
}

impl std::error::Error for IcfMeasurementError {}

/// A measured ideographic character-face anchor bound to the exact
/// [`RhythmFont`] it was measured from.
///
/// This capability exists only after successful [`RhythmFont::measure_icf`],
/// so its spacing operations are infallible and its ascent cannot be paired
/// with a different face, size, or rhythm grid. Cap height, when present,
/// arrives with a resolved [`RhythmFont`] and therefore exposes
/// [`RhythmFont::cap_span`] there; an ICF ascent depends on caller-selected
/// probes, which is why its `span` lives on this measured capability instead.
#[derive(Debug, Clone)]
pub struct RhythmIcfAnchor {
    font: RhythmFont,
    ascent: Pixels,
}

impl RhythmIcfAnchor {
    /// The resolved font this anchor was measured from.
    #[inline]
    pub const fn font(&self) -> &RhythmFont {
        &self.font
    }

    /// The paired ideographic-ink opening and closing.
    ///
    /// The opening lands the character face's top edge on the `top`th grid
    /// line. The closing hands the trimmed band back so the block still spans
    /// whole rhythm rows for any number of wrapped lines.
    ///
    /// A renderer that already has a trusted character-face ascent — or cannot
    /// measure one through gpui — can call
    /// [`RhythmBlockMetrics::ink_anchored`] directly.
    #[inline]
    pub fn span(&self, top: i32, bottom: i32) -> (Pixels, Pixels) {
        let block = RhythmBlockMetrics::ink_anchored(
            self.font.line_metrics(),
            f32::from(self.ascent),
            top,
            bottom,
        );
        (px(block.opening()), px(block.closing()))
    }

    /// The invisible band between the line box's top edge and the measured
    /// ideographic character face.
    #[inline]
    pub fn trim_top(&self) -> Pixels {
        self.font.baseline_above() - self.ascent
    }
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
    /// Metrics come from the resolved primary font only — what an element
    /// that sets one text style paints with. A line that explicitly mixes
    /// fonts (bold/italic runs, inline code, an explicit CJK or emoji face)
    /// has one baseline placed from the maximum ascent and descent over its
    /// runs; shape it and place the result through
    /// [`RhythmLineMetrics`](crate::RhythmLineMetrics) so that shared
    /// baseline still lands on the grid. Glyph-level fallback is different:
    /// substituted glyphs borrow the primary font's baseline and never enter
    /// the line's ascent, so fallback text needs no special handling.
    ///
    /// # Panics
    ///
    /// Panics when the font weight is zero, negative, or non-finite, when
    /// `font_size` is zero, negative, or non-finite, or when `line_rhythms` is
    /// zero. Validation happens before the text system is queried.
    pub fn resolve(
        text_system: &TextSystem,
        font: Font,
        font_size: Pixels,
        line_rhythms: u32,
        grid: RhythmGrid,
    ) -> Self {
        assert_valid_font_request(&font, font_size, line_rhythms);
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
            font_id: Some(font_id),
            metrics,
            grid,
        }
    }

    /// Compatibility constructor for a Plumber/rhythm-sass `baseline-ratio`.
    /// Prefer [`Self::resolve`]; see [`FontRhythm::from_baseline_ratio`].
    ///
    /// # Panics
    ///
    /// Panics when the font weight is zero, negative, or non-finite, when
    /// `font_size` is zero, negative, or non-finite, when `line_rhythms` is
    /// zero, or when `baseline_ratio` is not strictly between 0 and 1.
    pub fn from_baseline_ratio(
        font: Font,
        font_size: Pixels,
        line_rhythms: u32,
        baseline_ratio: f32,
        grid: RhythmGrid,
    ) -> Self {
        assert_valid_font_request(&font, font_size, line_rhythms);
        Self {
            font,
            font_id: None,
            metrics: FontRhythm::from_baseline_ratio(
                font_size.into(),
                line_rhythms,
                baseline_ratio,
            ),
            grid,
        }
    }

    /// Measure an ideographic character-face anchor from this resolved font.
    ///
    /// The result owns a clone of this font together with the tallest ink over
    /// `probes`, which should be full-frame glyphs of the script being set
    /// (`"字永語国"` for han, `"곽뻠한"` for hangul). Bounds that do not
    /// straddle the alphabetic baseline are ignored because they cannot
    /// establish that full frame. Only the returned [`RhythmIcfAnchor`] exposes
    /// the infallible [`RhythmIcfAnchor::span`] and
    /// [`RhythmIcfAnchor::trim_top`] operations.
    ///
    /// Measuring beats reading the font's `BASE` table. `BASE` is absent from
    /// some faces (SimSong ships none), and where present its `icft` is a
    /// *declaration* the ink need not match: measured against their own
    /// tables, PingFang SC and Toppan Bunkyu Gothic agree within 0.003 em, but
    /// Apple SD Gothic Neo's hangul overshoots its declared `icft` by
    /// 0.054 em. A measurement adapts to the face and the script; a table
    /// lookup does not.
    ///
    /// Like a cap anchor, this anchors *one* ink envelope: glyphs that reach
    /// it land on the grid line and shorter ones sit below, exactly as Latin
    /// lowercase sits below a `cap_top` anchor. Kana are the case to know
    /// about — their dakuten ride 0.005–0.058 em above the han envelope, like
    /// Latin ascenders above cap height — so include kana in `probes` when
    /// setting Japanese that must not exceed the line.
    ///
    /// Measurement happens only at this boundary; spacing on a successful
    /// anchor stays pure geometry. Use the same [`TextSystem`] that resolved
    /// this font because its [`FontId`] is local to that text system. The base
    /// `RhythmFont` remains determined entirely by [`Self::spec`] and is safe to
    /// cache normally. Cache a measured anchor separately only when useful,
    /// keyed by both the spec and probes.
    ///
    /// Availability follows gpui's text backend. In gpui 0.2.2, CoreText and
    /// DirectWrite expose glyph ink bounds, while Linux returns advance-only
    /// placeholder bounds that produce [`IcfMeasurementError::NoUsableBounds`].
    /// When every bounds query fails instead, this returns
    /// [`IcfMeasurementError::NoProbeBounds`]. If failures and rejected bounds
    /// are mixed, the returned-bounds distinction wins and the result is
    /// `NoUsableBounds`. Probe with glyphs the resolved face actually covers:
    /// CoreText and Linux report a missing glyph as absent, while DirectWrite
    /// substitutes `.notdef`, whose box can pass for ink.
    pub fn measure_icf(
        &self,
        text_system: &TextSystem,
        probes: &str,
    ) -> Result<RhythmIcfAnchor, IcfMeasurementError> {
        let font_id = self.font_id.ok_or(IcfMeasurementError::UnresolvedFont)?;
        if probes.is_empty() {
            return Err(IcfMeasurementError::EmptyProbes);
        }
        let font_size = self.font_size();
        let ascent = select_icf_ascent(
            probes
                .chars()
                .map(|ch| text_system.typographic_bounds(font_id, font_size, ch)),
        )?;

        Ok(RhythmIcfAnchor {
            font: self.clone(),
            ascent: px(ascent),
        })
    }

    /// The requested gpui font configuration applied by
    /// [`RhythmStyled::rhythm_font`].
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// The font identity gpui resolved the metrics from — the fallback
    /// font's when the requested family was unavailable. `None` when the
    /// value was built without a text system
    /// ([`Self::from_baseline_ratio`]).
    ///
    /// A [`FontId`] is an index into the resolving `TextSystem`: compare it
    /// with shaped-run font ids from the same system, but do not persist it
    /// or carry it across windows or font registrations — use
    /// [`Self::spec`] for durable request keys. "Did fallback happen" has no
    /// precise reverse lookup: `TextSystem::get_font_for_id` returns one
    /// cached font request for an id, not an authoritative platform-face
    /// identity, and can be ambiguous when several requests resolve to the
    /// same face. Check `TextSystem::all_font_names` before resolving when
    /// using the exact family is a requirement.
    #[inline]
    pub const fn resolved_font_id(&self) -> Option<FontId> {
        self.font_id
    }

    /// The TextSystem resolution request that produced this font, as a
    /// hashable cache key. Returns `None` for values synthesized by
    /// [`Self::from_baseline_ratio`], because their ratio-derived metrics
    /// cannot be reproduced by [`RhythmFontSpec::resolve`].
    pub fn spec(&self) -> Option<RhythmFontSpec> {
        self.font_id?;
        Some(RhythmFontSpec {
            font: self.font.clone(),
            font_size: self.font_size(),
            line_rhythms: self.metrics.line_rhythms(),
            grid_size: self.grid.size(),
        })
    }

    /// This font's line placement as a [`RhythmLineMetrics`] — the same
    /// value a shaped line produces, so a direct-paint renderer places
    /// single-style text, empty lines, and mixed-run shaped lines through
    /// one code path.
    pub fn line_metrics(&self) -> RhythmLineMetrics {
        self.metrics.line_metrics(self.grid.rhythm())
    }

    /// The grid this font was resolved against.
    #[inline]
    pub const fn grid(&self) -> RhythmGrid {
        self.grid
    }

    /// Top spacing that lands the first baseline `n` rhythm units below the
    /// element's padding edge (rhythm-sass `baseline-top()` / `rhythm-bottom()`).
    ///
    /// Negative when `n × grid size` is smaller than
    /// [`baseline_above`](Self::baseline_above) — meaningful as a margin, not
    /// as a padding.
    #[inline]
    pub fn baseline_top(&self, n: i32) -> Pixels {
        px(self.metrics.baseline_top(self.grid.rhythm(), n))
    }

    /// Bottom spacing that puts the nth grid line below the last baseline at
    /// the element's padding edge (rhythm-sass `baseline-bottom()` /
    /// `rhythm-top()`).
    ///
    /// Negative when `n × grid size` is smaller than the baseline-to-bottom
    /// distance — meaningful as a margin, not as a padding.
    #[inline]
    pub fn baseline_bottom(&self, n: i32) -> Pixels {
        px(self.metrics.baseline_bottom(self.grid.rhythm(), n))
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
    #[inline]
    pub fn baseline_between(&self, below: &RhythmFont, n: i32) -> Pixels {
        assert_eq!(
            self.grid, below.grid,
            "both fonts must be resolved against the same grid size"
        );
        px(self
            .metrics
            .baseline_between(self.grid.rhythm(), &below.metrics, n))
    }

    /// Top spacing that lands the capitals' ink top — not the baseline — on
    /// the nth grid line, for optically-aligned openings. Close the block
    /// with [`Self::cap_bottom`], not [`Self::baseline_bottom`]; see
    /// [`FontRhythm::cap_top`] for the contract, or use [`Self::cap_span`] to get
    /// the pair in one call. `None` when the font has no usable cap height.
    ///
    /// CJK faces usually resolve with a cap height — for their embedded Latin
    /// glyphs — so on ideographic text this returns `Some` while trimming to
    /// the wrong ink; use [`Self::measure_icf`] for CJK openings.
    #[inline]
    pub fn cap_top(&self, n: i32) -> Option<Pixels> {
        self.metrics.cap_top(self.grid.rhythm(), n).map(px)
    }

    /// Bottom spacing pairing [`Self::cap_top`], returning the trimmed space
    /// so the block closes on whole rhythm rows; see [`FontRhythm::cap_bottom`].
    /// `None` when the font has no usable cap height.
    #[inline]
    pub fn cap_bottom(&self, n: i32) -> Option<Pixels> {
        self.metrics.cap_bottom(self.grid.rhythm(), n).map(px)
    }

    /// The cap-anchored opening as one paired value:
    /// `(cap_top(top), cap_bottom(bottom))`. Taking the pair from a single
    /// call keeps the matching anchors together and reduces the chance of
    /// closing a cap opening with [`Self::baseline_bottom`] by mistake.
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
    #[inline]
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
    #[inline]
    pub const fn metrics(&self) -> &FontRhythm {
        &self.metrics
    }

    /// The font size the metrics were resolved at.
    #[inline]
    pub fn font_size(&self) -> Pixels {
        px(self.metrics.font_size())
    }

    /// The rhythm line height: `line_rhythms × grid size`.
    #[inline]
    pub fn line_height(&self) -> Pixels {
        px(self.metrics.line_height(self.grid.rhythm()))
    }

    /// Distance from the top of the line box down to the baseline, as gpui will
    /// paint it. Useful for custom elements and debug overlays.
    #[inline]
    pub fn baseline_above(&self) -> Pixels {
        px(self.metrics.baseline_above(self.grid.rhythm()))
    }

    /// Distance from the baseline down to the bottom of the line box, as gpui
    /// will paint it — the counterpart of [`Self::baseline_above`].
    #[inline]
    pub fn baseline_below(&self) -> Pixels {
        px(self.metrics.baseline_below(self.grid.rhythm()))
    }

    /// Invisible space above the cap height; subtract from a top spacing (or apply
    /// as a negative margin) for CSS `text-box-trim`-style optical alignment.
    /// `None` when the metrics source has no usable cap height, including values
    /// created with [`Self::from_baseline_ratio`].
    #[inline]
    pub fn cap_trim_top(&self) -> Option<Pixels> {
        self.metrics.cap_trim_top(self.grid.rhythm()).map(px)
    }

    /// Like [`Self::cap_trim_top`] but trimming to the x-height. `None` when the
    /// metrics source has no usable x-height.
    #[inline]
    pub fn x_trim_top(&self) -> Option<Pixels> {
        self.metrics.x_trim_top(self.grid.rhythm()).map(px)
    }
}

/// The pre-resolve identity of a [`RhythmFont`]: the requested [`Font`],
/// size, line rhythms, and grid size as one hashable value — the cache key
/// for caller-owned typography catalogs.
///
/// The crate deliberately keeps no font cache (gpui already caches
/// `Font → FontId` and metrics); an app reusing resolved values across a
/// document owns the map and its invalidation. Register font families before
/// resolving any spec for them: gpui caches failed lookups, so clearing this
/// caller-owned map after late registration cannot repair an earlier miss.
/// Rebuild the map when typography settings change:
///
/// ```no_run
/// use std::collections::HashMap;
///
/// use gpui::{font, px, TextSystem};
/// use rhythm_gpui::{RhythmFont, RhythmFontSpec, RhythmGrid};
///
/// fn body(cache: &mut HashMap<RhythmFontSpec, RhythmFont>, ts: &TextSystem) -> RhythmFont {
///     let spec = RhythmFontSpec::new(font("Noto Serif"), px(16.), 3, RhythmGrid::new(px(8.)));
///     cache
///         .entry(spec.clone())
///         .or_insert_with(|| spec.resolve(ts))
///         .clone()
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RhythmFontSpec {
    font: Font,
    font_size: Pixels,
    line_rhythms: u32,
    grid_size: Pixels,
}

impl RhythmFontSpec {
    /// The spec for resolving `font` at `font_size` on `grid` with a
    /// `line_rhythms`-unit line height.
    ///
    /// # Panics
    ///
    /// Panics when the font weight is zero, negative, or non-finite, when
    /// `font_size` is zero, negative, or non-finite, or when `line_rhythms` is
    /// zero.
    pub fn new(font: Font, font_size: Pixels, line_rhythms: u32, grid: RhythmGrid) -> Self {
        assert_valid_font_request(&font, font_size, line_rhythms);
        Self {
            font,
            font_size,
            line_rhythms,
            grid_size: grid.size(),
        }
    }

    /// The requested gpui font configuration.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// The font size the metrics will be resolved at.
    #[inline]
    pub const fn font_size(&self) -> Pixels {
        self.font_size
    }

    /// Line height in whole rhythm units.
    #[inline]
    pub const fn line_rhythms(&self) -> u32 {
        self.line_rhythms
    }

    /// The grid the font will be bound to.
    #[inline]
    pub fn grid(&self) -> RhythmGrid {
        RhythmGrid::new(self.grid_size)
    }

    /// Resolve the spec into a [`RhythmFont`] — [`RhythmFont::resolve`] with
    /// this identity; see it for the fallback-resolution caveats.
    pub fn resolve(&self, text_system: &TextSystem) -> RhythmFont {
        RhythmFont::resolve(
            text_system,
            self.font.clone(),
            self.font_size,
            self.line_rhythms,
            self.grid(),
        )
    }

    /// Resolve this spec at a line height that also contains every font in
    /// `others` — the catalog-build step that makes "the line box holds the
    /// ink" true by construction rather than per shaped line.
    ///
    /// A line shapes to the maxima over its explicit font runs, so a style
    /// needs a line height covering the tallest mixture of every face its
    /// lines can draw on: the run faces (bold, inline code, an explicit CJK
    /// or emoji face) and the families gpui would resolve to if one were
    /// missing. gpui shapes all `TextRun`s in a line at one font size, so
    /// every spec must use this spec's `font_size` and grid. Compatibility is
    /// checked before any font is resolved, then the resolved metrics are
    /// folded with [`RhythmLineMetrics::covering`]. The result is *this*
    /// spec's font at the covering count — metrics, cap height, and baselines
    /// stay the primary face's, only the line height grows. Nothing is shaped,
    /// so the count is a startup constant and every block's height follows
    /// from its line count, which is what a virtualized renderer needs.
    ///
    /// Keep placing each shaped line with its own `ascent`/`descent` at this
    /// font's [`line_rhythms`](FontRhythm::line_rhythms); with the height
    /// settled here, no shaped mixture of the covered set outgrows it.
    /// Resolve the run faces at that same count when their own metrics
    /// are used for placement — [`spec`](RhythmFont::spec) on the returned
    /// font carries it, and reproduces these metrics as usual.
    ///
    /// ```no_run
    /// use gpui::{font, px, FontWeight, TextSystem};
    /// use rhythm_gpui::{RhythmFontSpec, RhythmGrid};
    ///
    /// fn body(text_system: &TextSystem) -> rhythm_gpui::RhythmFont {
    ///     let grid = RhythmGrid::new(px(8.));
    ///     let mut bold = font("Georgia");
    ///     bold.weight = FontWeight::BOLD;
    ///     RhythmFontSpec::new(font("Georgia"), px(16.), 3, grid).resolve_covering(
    ///         text_system,
    ///         &[
    ///             RhythmFontSpec::new(bold, px(16.), 3, grid),
    ///             RhythmFontSpec::new(font("Menlo"), px(16.), 3, grid),
    ///             RhythmFontSpec::new(font("Apple Color Emoji"), px(16.), 3, grid),
    ///         ],
    ///     )
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when a spec in `others` was built with a different font size or
    /// grid size.
    pub fn resolve_covering(
        &self,
        text_system: &TextSystem,
        others: &[RhythmFontSpec],
    ) -> RhythmFont {
        for spec in others {
            assert_eq!(
                spec.grid_size, self.grid_size,
                "every covering font spec must use the primary grid size"
            );
            assert_eq!(
                spec.font_size, self.font_size,
                "every covering font spec must use the primary font size"
            );
        }

        let primary = self.resolve(text_system);
        let mut covered = Vec::with_capacity(others.len() + 1);
        covered.push(primary.line_metrics());
        covered.extend(
            others
                .iter()
                .map(|spec| spec.resolve(text_system).line_metrics()),
        );
        let line_rhythms =
            RhythmLineMetrics::covering(&covered, self.grid().rhythm()).line_rhythms();

        if line_rhythms == self.line_rhythms {
            return primary;
        }
        Self {
            line_rhythms,
            ..self.clone()
        }
        .resolve(text_system)
    }
}

/// A drop cap bound to the grid: the cap face at the size solved by
/// [`FontRhythm::drop_cap`], plus the inset anchoring its baseline. Apply with
/// [`RhythmStyled::rhythm_drop_cap`]; for wrap-around text, measure the letter
/// with `shape_line` (see `drop_cap_paragraph` in the `recipes` example).
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
            .drop_cap(body.grid.rhythm(), probe.metrics(), lines);
        Self {
            font: RhythmFont {
                font,
                // resolve_font identifies the face, not the size, so the
                // probe's identity holds at the solved size.
                font_id: probe.font_id,
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
    #[inline]
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

    /// Paint the debug grid over this element while `show` is true. Pass the
    /// grid itself for the classic translucent red (`0xff78783f`), or
    /// [`grid.overlay(color)`](RhythmGrid::overlay) — optionally with
    /// [`phase`](RhythmOverlay::phase) — to customize the stripes through the
    /// same chainable toggle. Chain it after the content children so the
    /// stripes paint on top; the element's top edge becomes the grid origin,
    /// and the element's own position style is left untouched.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gpui::{div, prelude::*, px, rgba};
    /// use rhythm_gpui::{RhythmGrid, RhythmStyled};
    ///
    /// fn page(show_grid: bool) -> impl IntoElement {
    ///     let grid = RhythmGrid::new(px(8.));
    ///     div()
    ///         .child("…content on the grid…")
    ///         .rhythm_debug_overlay(grid, show_grid)
    /// }
    ///
    /// fn tinted_page(show_grid: bool) -> impl IntoElement {
    ///     let grid = RhythmGrid::new(px(8.));
    ///     div()
    ///         .child("…content on the grid…")
    ///         .rhythm_debug_overlay(grid.overlay(rgba(0x0969da33)), show_grid)
    /// }
    /// ```
    fn rhythm_debug_overlay(self, overlay: impl Into<RhythmOverlay>, show: bool) -> Self
    where
        Self: ParentElement,
    {
        if show {
            self.child(overlay.into())
        } else {
            self
        }
    }
}

impl<T: Styled> RhythmStyled for T {}

/// Create a [`RhythmOverlay`] painting every other grid row in `color` — the
/// rhythm-sass `draw-rhythms()` mixin. Add [`RhythmOverlay::phase`] for a
/// renderer that scrolls by painting at an offset.
pub fn rhythm_overlay(grid: RhythmGrid, color: impl Into<Hsla>) -> RhythmOverlay {
    RhythmOverlay {
        grid,
        color: color.into(),
        phase: px(0.),
    }
}

/// A `draw-rhythms` debug overlay: every other grid row in one color.
/// Place it as the last child of the container it should cover; it fills that
/// container and ignores mouse events. An ordinary gpui container already uses
/// relative positioning by default, so no extra `.relative()` call is needed,
/// and this element does not alter the container's position style.
///
/// The stripes start at the container's top edge. If the content scrolls a
/// container, put the overlay *inside* the scrolled wrapper so the grid moves
/// with the text; a renderer that scrolls by painting content at a computed
/// signed Y offset instead passes that same offset to [`phase`](Self::phase).
/// Either way only rows intersecting the visible region (the current content
/// mask) are painted, and each is clipped to the overlay's own bounds, so
/// covering a document far taller than the viewport costs
/// `O(viewport height / grid size)` per frame and never paints outside the
/// container it covers. Custom elements whose offset settles during prepaint
/// can call [`paint`](Self::paint) with a freshly phased value during their
/// paint stage instead of capturing stale state during render.
#[derive(IntoElement)]
pub struct RhythmOverlay {
    grid: RhythmGrid,
    color: Hsla,
    phase: Pixels,
}

impl RhythmOverlay {
    /// Translate the stripes by the same signed Y `offset` used to paint the
    /// content, for renderers that scroll without moving a scroll container.
    /// A negative offset moves both content and grid origin above the
    /// element; a positive offset moves them below it. In particular,
    /// `ScrollHandle::offset().y` can be passed through directly.
    ///
    /// This has the effect of translating the overlay with the content —
    /// without a wrapper element, and without an ancestor to clip it, since
    /// stripes are clipped to the overlay's own bounds.
    ///
    /// # Panics
    ///
    /// Panics when `offset` is not finite.
    #[must_use]
    pub fn phase(mut self, offset: Pixels) -> Self {
        assert!(
            f32::from(offset).is_finite(),
            "overlay phase must be finite"
        );
        self.phase = offset;
        self
    }

    /// Paint the configured stripes directly into `window` within `bounds`.
    ///
    /// This is the paint-stage counterpart to using the overlay as an element.
    /// It is intended for custom renderers whose signed content translation is
    /// not final until another element's prepaint has settled layout. Read that
    /// application-owned value during paint, apply it to a temporary overlay
    /// with [`phase`](Self::phase), then call this method; the crate stores no
    /// callback or mutable scroll state.
    ///
    /// `bounds` takes the covered container's role, which the element form
    /// fills in from the parent: stripe row 0 starts at `bounds.origin.y` plus
    /// the [`phase`](Self::phase), and nothing paints outside the box. The
    /// current content mask is applied exactly as for the element form, and
    /// only visible stripes are visited. Call this after painting the content
    /// the stripes should cover.
    ///
    /// ```no_run
    /// use gpui::{Bounds, Pixels, Window, px, rgba};
    /// use rhythm_gpui::RhythmGrid;
    ///
    /// fn paint_grid(
    ///     bounds: Bounds<Pixels>,
    ///     content_offset_y: Pixels,
    ///     window: &mut Window,
    /// ) {
    ///     RhythmGrid::new(px(8.))
    ///         .overlay(rgba(0xff78783f))
    ///         .phase(content_offset_y)
    ///         .paint(bounds, window);
    /// }
    /// ```
    pub fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let visible = bounds.intersect(&window.content_mask().bounds);
        if visible.size.height <= px(0.) || visible.size.width <= px(0.) {
            return;
        }

        // Apply the same signed translation as the painted content so the grid
        // origin stays attached to the document.
        let origin_y = overlay_origin_y(bounds.origin.y, self.phase);
        visible_stripes(
            origin_y,
            f32::from(self.grid.size()),
            f32::from(visible.origin.y),
            f32::from(visible.bottom()),
            |from, to| {
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.origin.x, px(from)),
                        size(bounds.size.width, px(to - from)),
                    ),
                    self.color,
                ));
            },
        );
    }
}

#[inline]
fn overlay_origin_y(element_top: Pixels, phase: Pixels) -> f32 {
    f32::from(element_top) + f32::from(phase)
}

/// A bare grid converts to its default debug appearance — the classic
/// translucent red (`0xff78783f`), zero phase — which is what lets
/// [`RhythmStyled::rhythm_debug_overlay`] accept the grid directly.
impl From<RhythmGrid> for RhythmOverlay {
    fn from(grid: RhythmGrid) -> Self {
        rhythm_overlay(grid, rgba(DEFAULT_RHYTHM_OVERLAY_RGBA))
    }
}

impl RenderOnce for RhythmOverlay {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| self.paint(bounds, window),
        )
        .absolute()
        .inset_0()
    }
}

/// The overlay's whole geometry: every other row of the grid rooted at
/// `origin_y` — including any signed content translation — reported to
/// `paint` as `(from, to)` spans clipped to the visible `[top, bottom)`.
///
/// Whole periods before the visible region are skipped arithmetically rather
/// than walked, so an overlay over a document far taller than the viewport
/// still costs `O(viewport height / stripe height)`, and the clipping keeps
/// a translated first row (or an overhanging last one) inside the element.
fn visible_stripes(
    origin_y: f32,
    stripe_height: f32,
    top: f32,
    bottom: f32,
    mut paint: impl FnMut(f32, f32),
) {
    let step = stripe_height * 2.0;
    let skipped = (((top - origin_y - stripe_height) / step).ceil()).max(0.0);
    let mut y = origin_y + step * skipped;
    while y < bottom {
        let from = y.max(top);
        let to = (y + stripe_height).min(bottom);
        if to > from {
            paint(from, to);
        }
        let next = y + step;
        if !next.is_finite() || next <= y {
            break;
        }
        y = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        font, AnyElement, FontFallbacks, FontFeatures, FontId, FontStyle, FontWeight, Position,
        StyleRefinement,
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
    fn pixels_mirrors_agree_with_the_math_layer() {
        let grid = RhythmGrid::new(px(8.0));
        assert_eq!(grid.rhythm(), Rhythm::new(8.0));
        assert_eq!(grid.spacing(5), px(40.0));
        assert_eq!(grid.height(5), grid.spacing(5));
        let line = grid.line_metrics(px(14.67), px(3.51), 3);
        assert_eq!(line.line_height_px(), px(line.line_height()));

        let target = grid.height(5);
        assert_eq!(
            line.paint_origin_for_px(target),
            px(line.paint_origin_for(target.into()))
        );

        let block = RhythmBlockMetrics::new(line, 3, 1);
        assert_eq!(block.first_baseline_px(), px(block.first_baseline()));
        let wide_row = i64::from(i32::MAX) * 4;
        assert_eq!(
            block.baseline_at_row_px(wide_row),
            px(block.baseline_at_row(wide_row))
        );
    }

    #[test]
    fn line_metrics_grid_helpers_match_the_math_layer() {
        let grid = RhythmGrid::new(px(8.0));
        let grown = grid.line_metrics_at_least(px(20.0), px(6.0), 3);
        assert_eq!(grown.line_rhythms(), 4);
        assert!(!grown.overflows_line_box());

        let body = grid.line_metrics(px(14.67), px(3.51), 3);
        let display = grid.line_metrics(px(28.8), px(6.4), 3);
        assert_eq!(
            grid.line_metrics_covering(&[body, display]),
            RhythmLineMetrics::covering(&[body, display], body.grid())
        );
    }

    #[test]
    fn spacing_methods_agree_with_the_math_layer() {
        // Each assertion names the math method the gpui form must reach, so a
        // forwarder wired to its sibling (top to bottom, cap to baseline)
        // fails here even though every value stays self-consistent.
        let grid = RhythmGrid::new(px(8.0));
        let rhythm = grid.rhythm();
        let metrics = FontRhythm::from_platform_metrics(16.0, 3, 14.67, -3.51, 11.09, 7.70);
        let body = RhythmFont {
            font: font("Example Serif"),
            font_id: None,
            metrics,
            grid,
        };
        let below = RhythmFont::from_baseline_ratio(font("Example Serif"), px(24.0), 4, 0.2, grid);

        assert_eq!(body.baseline_top(3), px(metrics.baseline_top(rhythm, 3)));
        assert_eq!(
            body.baseline_bottom(1),
            px(metrics.baseline_bottom(rhythm, 1))
        );
        assert_eq!(
            body.baseline_between(&below, 6),
            px(metrics.baseline_between(rhythm, below.metrics(), 6))
        );
        assert_eq!(body.cap_top(3), metrics.cap_top(rhythm, 3).map(px));
        assert_eq!(body.cap_bottom(0), metrics.cap_bottom(rhythm, 0).map(px));
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

        let color = rgba(0x0969da33);
        let hidden = CapturedChildren::default().rhythm_debug_overlay(grid.overlay(color), false);
        assert!(hidden.children.is_empty());
        let shown = CapturedChildren::default().rhythm_debug_overlay(grid.overlay(color), true);
        assert_eq!(shown.children.len(), 1);
    }

    #[test]
    fn rhythm_overlay_keeps_the_requested_grid_and_color() {
        let grid = RhythmGrid::new(px(10.0));
        let color: Hsla = rgba(0x0969da33).into();
        let overlay = rhythm_overlay(grid, color);
        assert_eq!(overlay.grid, grid);
        assert_eq!(overlay.color, color);
        let mut spans = Vec::new();
        visible_stripes(
            0.0,
            f32::from(overlay.grid.size()),
            0.0,
            45.0,
            |from, to| spans.push((from, to)),
        );
        assert_eq!(spans, [(0.0, 10.0), (20.0, 30.0), (40.0, 45.0)]);
        let factory = grid.overlay(color);
        assert_eq!(factory.grid, grid);
        assert_eq!(factory.color, color);
        assert_eq!(factory.phase, px(0.));
    }

    #[test]
    fn a_bare_grid_converts_to_the_default_red_overlay() {
        let overlay = RhythmOverlay::from(RhythmGrid::new(px(8.0)));
        assert_eq!(overlay.color, rgba(DEFAULT_RHYTHM_OVERLAY_RGBA).into());
        assert_eq!(overlay.phase, px(0.));
    }

    #[test]
    fn rhythm_drop_cap_anchors_with_a_relative_inset_not_a_margin() {
        let grid = RhythmGrid::new(px(8.0));
        let body = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);
        let solved = body.metrics().drop_cap(Rhythm::new(8.0), body.metrics(), 3);
        let cap = RhythmDropCap {
            font: RhythmFont {
                font: font("Example Serif"),
                font_id: None,
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
            font_id: None,
            metrics,
            grid,
        };

        let (pt, pb) = heading.cap_span(3, 0).expect("cap metrics are present");
        assert_eq!(Some(pt), heading.cap_top(3));
        assert_eq!(Some(pb), heading.cap_bottom(0));
        let rows = f32::from(pt + heading.line_height() + pb) / 8.0;
        assert!((rows - rows.round()).abs() < 1e-3);

        // `cap_span` composes the `Rhythm` anchors while the ICF anchor goes
        // through the block metrics; pinning both to the same value keeps the
        // two ink-anchor paths from drifting apart.
        let line = metrics.line_metrics(grid.rhythm());
        let block =
            RhythmBlockMetrics::ink_anchored(line, metrics.cap_height().expect("cap height"), 3, 0);
        assert!((f32::from(pt) - block.opening()).abs() < 1e-4);
        assert!((f32::from(pb) - block.closing()).abs() < 1e-4);
    }

    #[test]
    fn cap_spacing_returns_none_without_cap_height() {
        let grid = RhythmGrid::new(px(8.0));
        let font = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);

        assert_eq!(font.cap_top(3), None);
        assert_eq!(font.cap_bottom(1), None);
        assert_eq!(font.cap_span(3, 1), None);
    }

    /// PingFang SC at 16px on a 3-unit line — hhea 1.060/-0.340, with an OS/2
    /// cap of 0.860 that is a placeholder copy of `sTypoAscender` rather than
    /// its Latin `H` — carrying the ICF ascent that
    /// [`RhythmFont::measure_icf`] would have measured.
    const PINGFANG_ICF_16: f32 = 0.822 * 16.0;

    fn pingfang_icf_16(grid: RhythmGrid) -> RhythmIcfAnchor {
        RhythmIcfAnchor {
            font: RhythmFont {
                font: font("Example CJK"),
                font_id: None,
                metrics: FontRhythm::from_platform_metrics(16.0, 3, 16.96, -5.44, 13.76, 9.6),
                grid,
            },
            ascent: px(PINGFANG_ICF_16),
        }
    }

    #[test]
    fn icf_anchor_pairs_the_anchors_and_spans_whole_rows() {
        let grid = RhythmGrid::new(px(8.0));
        let anchor = pingfang_icf_16(grid);
        let body = anchor.font();

        let (pt, pb) = anchor.span(3, 0);
        let trim = anchor.trim_top();
        assert_eq!(pt, grid.height(3) - trim);
        assert_eq!(pb, trim);
        // The character face's top edge lands on the third grid line…
        assert_eq!(pt + trim, grid.height(3));
        // …and the pair still spans whole rhythm rows.
        let rows = f32::from(pt + body.line_height() + pb) / 8.0;
        assert!((rows - rows.round()).abs() < 1e-3);
        // The reported cap height anchors somewhere else entirely.
        assert_ne!(body.cap_top(3), Some(pt));
    }

    #[test]
    fn icf_anchor_agrees_with_the_generic_block_metrics() {
        // The gpui pair is a convenience over a capability the math layer
        // already has; if these ever disagree, one of them is wrong.
        let grid = RhythmGrid::new(px(8.0));
        let anchor = pingfang_icf_16(grid);
        let body = anchor.font();

        let (pt, pb) = anchor.span(3, 0);
        let line = body.metrics().line_metrics(grid.rhythm());
        let block = RhythmBlockMetrics::ink_anchored(line, PINGFANG_ICF_16, 3, 0);
        assert!((f32::from(pt) - block.opening()).abs() < 1e-4);
        assert!((f32::from(pb) - block.closing()).abs() < 1e-4);
    }

    #[test]
    fn icf_measurement_rejects_advance_only_placeholder_bounds() {
        let real_ink = Bounds {
            origin: point(px(0.0), px(-1.6)),
            size: size(px(16.0), px(14.8)),
        };
        assert!((ideographic_ink_ascent(real_ink).unwrap() - 13.2).abs() < 1e-4);

        // gpui 0.2.2's Linux backend currently returns an advance rectangle
        // rooted at zero instead of glyph ink bounds. A nonzero vertical
        // advance must not become a plausible-looking ICF ascent.
        let advance_only = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(16.0), px(16.0)),
        };
        assert_eq!(ideographic_ink_ascent(advance_only), None);

        assert_eq!(
            select_icf_ascent([Err::<Bounds<Pixels>, ()>(())]),
            Err(IcfMeasurementError::NoProbeBounds)
        );
        assert_eq!(
            select_icf_ascent([Ok::<Bounds<Pixels>, ()>(advance_only)]),
            Err(IcfMeasurementError::NoUsableBounds)
        );
        assert_eq!(
            select_icf_ascent([Err(()), Ok(advance_only)]),
            Err(IcfMeasurementError::NoUsableBounds),
            "a returned-but-rejected bound wins over failed probe queries"
        );
        assert!(
            (select_icf_ascent([Err(()), Ok(real_ink)]).unwrap() - 13.2).abs() < 1e-4,
            "one usable probe makes the aggregate measurement succeed"
        );
    }

    #[test]
    #[should_panic(expected = "rhythm unit size must be finite and greater than zero")]
    fn grid_rejects_a_non_positive_size() {
        let _ = RhythmGrid::new(px(0.0));
    }

    /// The overlay's painted spans for an 8px grid rooted at `origin_y`,
    /// over the visible region `[top, bottom)`.
    fn stripe_spans(origin_y: f32, top: f32, bottom: f32) -> Vec<(f32, f32)> {
        let mut spans = Vec::new();
        visible_stripes(origin_y, 8.0, top, bottom, |from, to| {
            spans.push((from, to))
        });
        spans
    }

    #[test]
    fn overlay_stripes_every_other_row_from_the_container_top() {
        assert_eq!(
            stripe_spans(0.0, 0.0, 40.0),
            [(0.0, 8.0), (16.0, 24.0), (32.0, 40.0)]
        );
        // The last row is clipped to the element instead of bleeding past it.
        assert_eq!(
            stripe_spans(0.0, 0.0, 36.0),
            [(0.0, 8.0), (16.0, 24.0), (32.0, 36.0)]
        );
    }

    #[test]
    fn overlay_walks_only_the_visible_region_and_keeps_the_phase() {
        // A row still partially visible at the mask's top edge is kept and
        // clipped to it; a whole 1600px of scrolled-past rows costs no work.
        assert_eq!(
            stripe_spans(0.0, 1607.0, 1630.0),
            [(1607.0, 1608.0), (1616.0, 1624.0)]
        );
        // One pixel later that row is gone, and the phase still holds.
        assert_eq!(stripe_spans(0.0, 1609.0, 1630.0), [(1616.0, 1624.0)]);
    }

    #[test]
    fn overlay_phase_uses_the_contents_signed_translation() {
        let grid = RhythmGrid::new(px(8.0));

        // GPUI scroll offsets become negative as content moves up. Passing
        // that value through directly moves the overlay origin the same way:
        // after one row the opening stripe is gone and the next starts at 8.
        let one_row = rhythm_overlay(grid, rgba(0xff78783f)).phase(px(-8.0));
        let one_row_origin = overlay_origin_y(px(0.0), one_row.phase);
        assert_eq!(one_row_origin, -8.0);
        assert_eq!(
            stripe_spans(one_row_origin, 0.0, 40.0),
            [(8.0, 16.0), (24.0, 32.0)]
        );

        // Half a row leaves the visible half of the translated stripe at the
        // element's top edge — no ancestor clipping needed.
        let half_row = rhythm_overlay(grid, rgba(0xff78783f)).phase(px(-4.0));
        let half_row_origin = overlay_origin_y(px(0.0), half_row.phase);
        assert_eq!(
            stripe_spans(half_row_origin, 0.0, 40.0),
            [(0.0, 4.0), (12.0, 20.0), (28.0, 36.0)]
        );

        // A positive content translation moves the document origin down and
        // leaves the area before it unpainted.
        let down = rhythm_overlay(grid, rgba(0xff78783f)).phase(px(4.0));
        let down_origin = overlay_origin_y(px(0.0), down.phase);
        assert_eq!(down_origin, 4.0);
        assert_eq!(
            stripe_spans(down_origin, 0.0, 24.0),
            [(4.0, 12.0), (20.0, 24.0)]
        );

        // Deep into a scrolled document the signed phase is still exact, and
        // only the visible period is walked.
        let deep = rhythm_overlay(grid, rgba(0xff78783f)).phase(px(-1600.0));
        let deep_origin = overlay_origin_y(px(0.0), deep.phase);
        assert_eq!(
            stripe_spans(deep_origin, 0.0, 24.0),
            [(0.0, 8.0), (16.0, 24.0)]
        );
    }

    #[test]
    fn overlay_stops_when_a_stride_is_below_coordinate_precision() {
        // Both values are valid f32 coordinates, but adding this period at
        // y=100 rounds back to 100. The paint loop must stop instead of
        // waiting forever for a representable next stripe.
        assert_eq!(100.0f32 + 2.0e-7, 100.0);
        let mut spans = Vec::new();
        visible_stripes(0.0, 1.0e-7, 100.0, 101.0, |from, to| spans.push((from, to)));
        assert!(spans.is_empty());
    }

    #[test]
    #[should_panic(expected = "overlay phase must be finite")]
    fn overlay_rejects_a_non_finite_phase() {
        let _ = rhythm_overlay(RhythmGrid::new(px(8.0)), rgba(0xff78783f)).phase(px(f32::NAN));
    }

    #[test]
    fn covering_reaches_the_math_layer_from_grid_built_lines() {
        let grid = RhythmGrid::new(px(8.0));
        let body = grid.line_metrics(px(14.67), px(3.51), 3);
        let display = grid.line_metrics(px(28.8), px(6.4), 3);
        let covering = grid.line_metrics_covering(&[body, display]);

        // Three rows cannot hold the display face's ink; five can, and every
        // mixture of the pair fits that count.
        assert_eq!(covering.line_rhythms(), 5);
        assert!(!grid
            .line_metrics(px(28.8), px(6.4), covering.line_rhythms())
            .overflows_line_box());
    }

    #[test]
    fn spec_keys_a_map_for_text_system_resolved_fonts() {
        use std::collections::HashMap;

        let grid = RhythmGrid::new(px(8.0));
        let metrics = FontRhythm::from_baseline_ratio(16.0, 3, 0.2);
        let body = RhythmFont {
            font: font("Example Serif"),
            font_id: Some(FontId(7)),
            metrics,
            grid,
        };
        let spec = body.spec().expect("font came from a text system");
        assert_eq!(
            spec,
            RhythmFontSpec::new(font("Example Serif"), px(16.0), 3, grid)
        );
        assert_eq!(spec.font(), body.font());
        assert_eq!(spec.font_size(), body.font_size());
        assert_eq!(spec.line_rhythms(), 3);
        assert_eq!(spec.grid(), grid);

        let mut cache = HashMap::new();
        cache.insert(spec.clone(), body);
        assert!(cache.contains_key(&spec));
        // A different size is a different identity.
        let other = RhythmFontSpec::new(font("Example Serif"), px(17.0), 3, grid);
        assert!(!cache.contains_key(&other));
    }

    #[test]
    fn spec_rejects_invalid_request_values_before_it_can_be_a_cache_key() {
        let grid = RhythmGrid::new(px(8.0));
        for invalid in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
            let result = std::panic::catch_unwind(|| {
                RhythmFontSpec::new(font("Example Serif"), px(invalid), 3, grid)
            });
            assert!(result.is_err(), "accepted invalid font size {invalid:?}");
        }
        let zero_lines = std::panic::catch_unwind(|| {
            RhythmFontSpec::new(font("Example Serif"), px(16.0), 0, grid)
        });
        assert!(zero_lines.is_err(), "accepted a zero line-rhythm count");

        for invalid in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut invalid_font = font("Example Serif");
            invalid_font.weight = FontWeight(invalid);
            let result =
                std::panic::catch_unwind(|| RhythmFontSpec::new(invalid_font, px(16.0), 3, grid));
            assert!(result.is_err(), "accepted invalid font weight {invalid:?}");
        }
    }

    #[test]
    fn baseline_ratio_font_rejects_an_invalid_font_weight() {
        let grid = RhythmGrid::new(px(8.0));
        for invalid in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut invalid_font = font("Example Serif");
            invalid_font.weight = FontWeight(invalid);
            let result = std::panic::catch_unwind(|| {
                RhythmFont::from_baseline_ratio(invalid_font, px(16.0), 3, 0.2, grid)
            });
            assert!(result.is_err(), "accepted invalid font weight {invalid:?}");
        }
    }

    #[test]
    fn baseline_ratio_fonts_carry_no_resolved_identity() {
        let grid = RhythmGrid::new(px(8.0));
        let first = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);
        let second = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.3, grid);
        assert_eq!(first.resolved_font_id(), None);
        assert_eq!(first.spec(), None);
        assert_eq!(second.spec(), None);
        assert_ne!(first.metrics(), second.metrics());
    }

    #[test]
    fn line_metric_factories_agree_with_the_math_layer() {
        let grid = RhythmGrid::new(px(8.0));
        let body = RhythmFont::from_baseline_ratio(font("Example Serif"), px(16.0), 3, 0.2, grid);
        let from_font = body.line_metrics();
        let from_grid =
            grid.line_metrics(px(body.metrics().ascent()), px(body.metrics().descent()), 3);
        assert_eq!(from_font, from_grid);
        assert_eq!(f32::from(body.baseline_above()), from_font.baseline_above());
        assert_eq!(f32::from(body.baseline_below()), from_font.baseline_below());
        assert_eq!(f32::from(body.line_height()), from_font.line_height());
    }
}
