# Changelog

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
