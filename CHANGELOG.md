# Changelog

## 0.2.0 (unreleased)

API reshaped along the ownership chain: a `RhythmFont` already carries its
grid, so spacing now comes from the font itself and the grid acts as the
factory. Passing the grid alongside a font — and the runtime mismatch panic
that guarded it — is gone from the new paths.

- Spacing moved to `RhythmFont`: `font.baseline_top(n)`,
  `font.baseline_bottom(n)`, `above.baseline_between(&below, n)`,
  `font.cap_top(n)`, `font.cap_bottom(m)`. The `RhythmGrid` equivalents are
  deprecated and forward to the new methods.
- Factory constructors: `grid.font(ts, font, size, line_rhythms)` and
  `body.drop_cap(ts, font, lines)`. `RhythmFont::resolve` and
  `RhythmDropCap::resolve` remain as the explicit forms.
- `RhythmFont::cap_span(top, bottom)` returns the cap-anchored padding pair as
  one `Option`, making a cap opening closed with a baseline `bottom` — the
  off-grid mistake the docs used to warn about — inexpressible.
- `RhythmStyled::rhythm_block(&font, top, bottom)`: the whole-rows text-block
  recipe (font plus baseline-paired paddings) in one call.
- `RhythmFont::grid()` accessor.

Media support — content whose height follows its width (images, video,
embeds) no longer knocks everything after it off the grid:

- `Rhythm::snap_up` / `snap_down` (and the `Pixels`-typed `RhythmGrid`
  wrappers): round a free height outward to whole rhythm rows, with a small
  `f32`-precision tolerance absorbing float error from measured sizes.
- `rhythm_frame(grid, ratio)`: a fluid-width container that re-snaps its
  height in the layout pass at every width. Default pads the sub-unit
  remainder below the content; `.crop()` rounds down and clips natural-height
  content evenly between its top and bottom edges. The content overlays a
  measured sizer leaf, so the box reserves its height before an image loads —
  no layout shift.
- Demo: a fluid-width image with a pad/crop toolbar toggle.

Shaped-line placement — a line mixing explicit font runs (bold/italic,
inline code, an explicit CJK or emoji face) shapes to the maximum
ascent/descent over its runs, which primary-font metrics cannot describe.
A new value-only layer makes that the supported path for custom document
renderers, and retires the old "lay each font out as its own element"
advice:

- `RhythmLineMetrics`: placement from a shaped line's real
  `ascent()`/`descent()` — `line_height`, `baseline_above`/`baseline_below`,
  `half_leading`, `paint_origin_for(target_baseline)`, and the advisory
  `min_line_rhythms` / `overflows_line_box` pair for ink taller than the
  chosen line box. Factories: `RhythmGrid::line_metrics(ascent, descent, n)`,
  `RhythmFont::line_metrics()`, `FontRhythm::line_metrics(grid)`.
- `RhythmBlockMetrics`: whole-row block geometry over one line's metrics —
  baseline anchors (`new`) or cap-ink anchors (`cap`, the pure form of the
  `cap_top`/`cap_bottom` pair), `opening`/`closing`, `first_baseline`, exact
  integer `rows`, and first/middle/last fragment heights that keep the
  rhythm phase across virtualized splits.
- Glyph-level fallback still needs no special handling: substituted glyphs
  never enter a line's shaped ascent (CoreText-verified).
- `direct_paint` example: resolve a font set once, shape and cache
  `WrappedLine`s, place them with the metrics types, and paint directly.

Resolved-font lifecycle, now a documented contract — within the crate, only
the resolution factories touch the `TextSystem`; the `Rhythm`, `FontRhythm`,
and line/block geometry paths afterwards are allocation-free, lock-free
`Copy` math (enforced by a counting-allocator test). `cargo bench --bench
resolve` tracks warm resolution, deliberately without ns-level CI thresholds:

- `RhythmFontSpec`: the pre-resolve identity (`Font`, size, line rhythms,
  grid size) as an `Eq + Hash` cache key, with `spec.resolve(ts)` and
  `font.spec()` for TextSystem-resolved values (`None` for synthesized
  baseline-ratio values). The crate keeps no font cache of its own. Families
  must be registered before first resolution because gpui caches failed font
  requests inside the `TextSystem`.
- `RhythmFont::resolved_font_id()`: the `FontId` the metrics came from
  (`None` for `from_baseline_ratio` values). Valid only in the resolving
  `TextSystem`; there is intentionally no `was_fallback` flag, since gpui
  cannot answer that precisely.

Verification and fixes:

- Real-shaping integration suite (`tests/shaping.rs`, macOS/CoreText, its
  own binary because AppKit needs the main thread): single-run metric parity
  with an OpenType descent-sign guard, mixed-run maxima, glyph-fallback
  non-inflation, and overflow behavior, using bundled OFL Noto fonts
  (`tests/fonts/`, excluded from the published package — the suite
  self-skips without them). Found upstream: `TextSystem::baseline_offset`
  disagrees with the paint path by one descent on macOS (raw negative
  descent vs shaped positive), so the paint equation is the oracle instead.
- `rhythm_overlay` now paints only the rows intersecting the visible region
  (content mask), keeping the stripe phase anchored to the container — debug
  overlays on very tall documents cost `O(viewport)`, not `O(document)`.
- Platform scope: mixed-run and glyph-fallback semantics are verified on
  macOS/CoreText only; other gpui text backends are not assumed identical.

## 0.1.0 (2026-08-15)

Initial release: vertical rhythm typography for gpui, ported from
[rhythm-sass](https://github.com/p233/rhythm-sass).

- Dependency-free rhythm math (`Rhythm`, `FontRhythm`, `DropCapRhythm`, `snap`),
  usable without gpui via `default-features = false`.
- gpui integration: `RhythmGrid`, `RhythmFont` (metrics resolved from the actual
  font file — no hand-measured baseline-ratio), `RhythmStyled`, `rhythm_overlay`,
  and the chainable `rhythm_debug_overlay` dev toggle.
- Drop caps: `FontRhythm::drop_cap` solves the cap size and baseline anchor from
  the cap face's own metrics; `RhythmDropCap::resolve` / `rhythm_drop_cap` apply
  it in gpui as a relative inset, so the flex row keeps its cross size even for
  cap-heavy faces whose anchor is a downward shift (e.g. Merriweather).
- Optical cap anchoring: `cap_top` lands the capitals' ink on a grid line and
  the paired `cap_bottom` closes the block on whole rhythm rows; `None` for
  faces without a usable cap height.
- Sass-parity tests against the rhythm-sass fixture values; platform metric
  normalization (OpenType negative-descent convention) with regression tests.
- Example: `demo` — on-demand Google Fonts, a three-line drop cap with true
  wrap-around, a mixed-font baseline row, a baseline/cap heading anchor
  toggle, and the grid overlay.
