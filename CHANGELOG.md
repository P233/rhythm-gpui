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
