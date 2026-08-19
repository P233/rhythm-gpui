# Changelog

## 0.2.0 (unreleased)

API reshaped along the ownership chain: a `RhythmFont` already carries its
grid, so spacing now comes from the font itself and the grid acts as the
factory. Passing the grid alongside a font — and the runtime mismatch panic
that guarded it — is gone.

- Spacing moved to `RhythmFont`: `font.baseline_top(n)`,
  `font.baseline_bottom(n)`, `above.baseline_between(&below, n)`,
  `font.cap_top(n)`, `font.cap_bottom(m)`. The `RhythmGrid` equivalents —
  which took a font alongside the grid and panicked when the two disagreed —
  are removed, and with them the grid/font mismatch check they existed to
  guard. Two grid-agreement checks remain, both where two values that each
  carry a grid meet: `RhythmFont::baseline_between` and
  `RhythmLineMetrics::covering`. Everywhere else the mismatch is now
  unrepresentable rather than checked for.
- The dependency-free layer follows the same split:
  `FontRhythm::baseline_top(grid, n)`, `baseline_bottom(grid, n)`,
  `baseline_between(grid, &below, n)`, `cap_top(grid, n)` and
  `cap_bottom(grid, n)` replace the `Rhythm` methods of those names. `Rhythm`
  is now purely the grid — size, spacing, height, snapping — and every
  font-dependent geometry lives on `FontRhythm`. A `FontRhythm` carries no
  grid, so this moves the receiver rather than removing an argument; the gain
  is that one rule now says where a calculation lives. `FontRhythm::drop_cap`
  takes the grid first for the same reason: every `FontRhythm` method that
  needs a grid now names it in the same position.
- `FontRhythm::with_cap_height` / `with_x_height` are removed.
  `from_platform_metrics` already supplies both heights alongside the metrics
  they belong to, and the setters were a second entry into the same state.
  Pass `0.0` for a height you do not have — non-finite and non-positive cap
  and x heights are treated as unavailable. From a bare baseline ratio, the
  em-box formula `from_baseline_ratio` documents composes the same value:
  `from_platform_metrics(size, rhythms, size * (1.0 - ratio), size * ratio,
cap, x)`.
- Factory constructors: `grid.font(ts, font, size, line_rhythms)` and
  `body.drop_cap(ts, font, lines)`. `RhythmFont::resolve` and
  `RhythmDropCap::resolve` remain as the explicit forms.
- `RhythmFont::cap_span(top, bottom)` returns the cap-anchored padding pair as
  one `Option`, keeping the matching anchors together and making a cap opening
  accidentally closed with a baseline `bottom` less likely.
- `RhythmStyled::rhythm_block(&font, top, bottom)`: the whole-rows text-block
  recipe (font plus baseline-paired paddings) in one call.
- `RhythmFont::grid()`, `RhythmFont::baseline_below()` (the counterpart of the
  existing `baseline_above()`), and `RhythmGrid::rhythm()` — the entry to the
  dependency-free `Rhythm`, mirroring `RhythmFont::metrics()` for fonts.
- `Rhythm::spacing(n)` / `RhythmGrid::spacing(n)`: an axis-neutral rhythm-unit
  length for horizontal padding, gaps, and indents; `height(n)` remains the
  vertical name for the same calculation.
- `RhythmStyled::rhythm_debug_overlay` now takes `impl Into<RhythmOverlay>`
  instead of `RhythmGrid`. Passing the grid still compiles and keeps the
  classic translucent red (`0xff78783f`); the new `grid.overlay(color)`
  factory — optionally with `.phase(offset)` — customizes the stripes through
  the same chainable toggle. `rhythm_overlay(grid, color)` remains the
  explicit form. `RhythmOverlay::paint(bounds, window)` exposes the identical
  clipping and visible-stripe walk to custom elements whose content translation
  only settles during prepaint; the application supplies the final phase at
  paint time and the crate stores no callback or scroll state.

CJK ink anchors — horizontal CJK already shared Latin's baseline, so
`baseline_top` and friends needed no change and none was made. One thing
baseline anchoring cannot do is land ideographic _ink_ on a grid line, and
that is the whole of this addition:

- `RhythmFont::measure_icf(text_system, probes)` measures the character face
  from the resolved face through `typographic_bounds` and returns a
  `RhythmIcfAnchor` bound to that exact font, size, and grid. Its infallible
  `span(top, bottom)` returns the paired opening that lands ideographic ink on
  a grid line and still spans whole rhythm rows; `trim_top()` reports the
  invisible band between the line box and that ink. `RhythmFont` itself remains
  entirely determined by `RhythmFontSpec`, so ordinary resolved-font caches
  carry no hidden measured/unmeasured state. The math layer needs nothing new:
  `RhythmBlockMetrics::ink_anchored` takes an anchored ink ascent — a Latin cap
  height or a CJK character-face ascent — and yields the same opening and
  closing, plus row and fragment geometry.
  Measuring beats reading the font's `BASE` table: SimSong ships none, and
  Apple SD Gothic Neo's hangul overshoots its declared `icft` by 0.054 em,
  while PingFang SC and Toppan Bunkyu Gothic agree within 0.003 em. Pass a
  _set_ of full-frame glyphs — 国 alone stops 0.05 em short of 字 — and include
  kana for Japanese, whose dakuten ride above the han envelope as Latin
  ascenders ride above cap height. The measurement remains optional: gpui
  0.2.2 exposes glyph ink bounds on CoreText and DirectWrite, while its Linux
  backend does not yet implement them. Measurement preserves the public
  distinction instead of discarding it: empty input returns `EmptyProbes`, all
  failed bounds queries return `NoProbeBounds`, and returned-but-rejected bounds
  return `NoUsableBounds` (the Linux path). DirectWrite can still substitute a
  missing glyph with `.notdef`, so callers must choose probes covered by the
  resolved face.
- Cap-anchor docs now warn that a CJK face's reported cap and x heights may
  describe no glyph at all: PingFang's `sCapHeight` 0.860 em is a copy of its
  `sTypoAscender` while its `H` reaches 0.714 em.
- Deliberately absent, each for a reason now in the README: no Latin/CJK
  layout mode (both scripts share one baseline; which leads is a per-block
  anchor choice), no cross-script size solver (a CJK face's own Latin is
  already balanced by its designer — PingFang draws `H` at 0.773 of its
  character face — and a deliberate pairing is one line of arithmetic), and no
  CJK drop cap (a Latin convention; CJK paragraphs open with a first-line
  indent).
- `recipes` example: twin ruled cards drawing the target so naive `.pt()`
  visibly misses it while a heading anchored on its measured character face
  lands on it, every edge a grid citizen so the overlay checks the claim. The
  mixed-font row now closes each span with its own `baseline_bottom`, so it
  spans whole rhythm rows.
- `tests/shaping.rs` gains an ideographic-ink group pinning the measured
  values against the font tables, the whole-row pairing, the anchor's resolved
  font identity, the `EmptyProbes` / `UnresolvedFont` boundaries, and
  CoreText's missing-glyph path to `NoProbeBounds`, plus the kana overshoot and
  measurement for a face with no `BASE` table.

Media support — content whose height follows its width (images, video,
embeds) no longer knocks everything after it off the grid:

- `Rhythm::snap_up` / `snap_down` (and the `Pixels`-typed `RhythmGrid`
  wrappers): round a free height outward to whole rhythm rows, with a small
  `f32`-precision tolerance absorbing float error from measured sizes.
- `rhythm_frame(grid, ratio)`: a fluid-width container that re-snaps its
  height in the layout pass at every width. `.fit(RhythmFit::Pad | Crop)`
  chooses the mode as a value, so a runtime choice stays one call; the default
  pads the sub-unit remainder below the content, and `RhythmFit::Crop` (or the
  `.crop()` shorthand) rounds down and clips natural-height content evenly
  between its top and bottom edges. The frame requires a parent offering a
  definite width. The content overlays a
  measured sizer leaf, so the box reserves its height before an image loads —
  no layout shift.
- `recipes` example (renamed from `demo` to say what it collects): a
  fluid-width 21:9 image with a pad/crop toolbar toggle.
- Toolbar controls in the `recipes` example now participate in one Tab /
  Shift-Tab order and reuse
  their click handlers for Enter/Space activation. Font downloads reject an
  oversized response explicitly instead of registering truncated bytes.

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
  baseline anchors (`new`) or ink-top anchors (`ink_anchored`), opening/closing and
  first/middle/last fragment heights, `first_baseline`, the
  unit-explicit `top_rhythms`/`bottom_rhythms` anchor counts (matching
  `line_rhythms`, since the neighboring accessors return lengths), and exact
  integer `rows`, so callers can use concrete geometry without accumulated
  `f32` drift or keep a row cursor for virtualization.
- `RhythmBlockMetrics::first_rows` / `middle_rows` / `last_rows`: ordered
  integer cursor transitions that partition `rows` precisely across split
  blocks. Accumulate them in an `i64`; `baseline_at_row` restores a rebased
  visible baseline without an `i32` saturation path and retains ink-anchor
  phase.
- `RhythmLineMetrics::at_least` and `RhythmGrid::line_metrics_at_least`: grow a
  dynamic line box when its configured height is a floor rather than a fixed
  virtualization budget.
- `RhythmLineMetrics::covering` (with `RhythmGrid::line_metrics_covering` and
  `RhythmFontSpec::resolve_covering` as typed and resolution-time entries): fix
  a style's line height over the
  whole same-size, same-grid set of faces its lines can draw on — its own, its
  run faces, and the families gpui would resolve to if one were missing —
  instead of per shaped line. Since a line shapes to the maxima over its runs,
  no mixture of a covered set can overflow the covering box, so "the line box
  contains the ink" becomes a property of construction. Nothing is shaped to
  compute it: the row budget is a startup constant, which is what lets a
  virtualized renderer derive a block's height from its line count alone.
  `resolve_covering` grows only the line height — metrics, cap height, and
  baselines stay the primary face's, and the returned font's `spec()` still
  reproduces it.
- `Pixels`-typed `_px` mirrors under the `gpui` feature for the four values
  that stay inside a paint path's `Pixels` chain: `line_height_px`,
  `paint_origin_for_px`, `first_baseline_px`, and the new wide-cursor
  `baseline_at_row_px`. The rest of the line and block geometry is read once
  and deliberately not mirrored — one `px(...)` at the call site beats a
  mirror per accessor.
- Glyph-level fallback still needs no special handling: substituted glyphs
  never enter a line's shaped ascent (CoreText-verified).
- `direct_paint` example: resolve a font set once — the body style covering
  its run faces — shape and cache `WrappedLine`s, place them with the metrics
  types at that fixed row budget, and paint directly.

Resolved-font lifecycle, now a documented contract — within the crate, only
the resolution factories touch the `TextSystem`; the `Rhythm`, `FontRhythm`,
and line/block geometry paths afterwards are allocation-free `Copy` math
(enforced by a counting-allocator test) and contain no locking. `cargo bench
--bench resolve` tracks warm resolution, deliberately without ns-level CI thresholds:

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
  self-skips without them), plus a covering-budget group: a line height
  resolved over a face set really does hold every mixture those faces shape
  to, settled before anything is shaped. Found upstream:
  `TextSystem::baseline_offset`
  disagrees with the paint path by one descent on macOS (raw negative
  descent vs shaped positive), so the paint equation is the oracle instead.
- `rhythm_overlay` now paints only the rows intersecting the visible region
  (content mask), keeping the stripe phase anchored to the container — debug
  overlays on very tall documents cost `O(viewport)`, not `O(document)`.
- `rhythm_overlay` returns a named, exported `RhythmOverlay` element carrying
  `.phase(offset)`: the stripe pattern takes the same signed Y translation as
  content painted at a computed offset instead of moving a scroll container —
  including gpui's negative scroll offsets — so the translated wrapper that
  job used to need is gone. Stripes are clipped to the overlay's own bounds,
  so a translated first row (and an overhanging last one) no longer depends on
  an ancestor's `overflow_hidden`.
- Platform scope: mixed-run and glyph-fallback semantics are verified on
  macOS/CoreText only; other gpui text backends are not assumed identical.
  The default gpui integration is additionally compile-checked on Linux and
  Windows, and the wide-cursor suite covers a multi-block viewport rebase past
  the `i32` document range.

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
