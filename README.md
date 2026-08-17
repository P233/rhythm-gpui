# rhythm-gpui

[![CI](https://github.com/p233/rhythm-gpui/actions/workflows/ci.yml/badge.svg)](https://github.com/p233/rhythm-gpui/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rhythm-gpui.svg)](https://crates.io/crates/rhythm-gpui)
[![docs.rs](https://img.shields.io/docsrs/rhythm-gpui)](https://docs.rs/rhythm-gpui)

Print-inspired vertical rhythm for [gpui](https://www.gpui.rs) — baseline
offsets computed from real font metrics. Ported from
[rhythm-sass](https://github.com/p233/rhythm-sass), its baseline-anchored paths
land text baselines exactly on a vertical rhythm grid. Cap-anchored openings
instead pin the capitals' ink while keeping each block on whole rhythm rows.

Unlike the Sass original, **no hand-measured `baseline-ratio` is required**:
metrics are read from the actual font file through gpui's text system.

![The recipes example with the grid overlay on: a three-line drop cap, fluid-width media in pad mode, and four text runs at different sizes and scripts sharing one baseline, all composed on the grid](assets/recipes.png)

_`cargo run --example recipes` — switch the typeface and baseline-anchored text
keeps its appointment with the grid: the drop cap spans three lines,
fluid-width media pads or crops to whole rows at every window width, and four
runs (two serif sizes, monospace, CJK) share a single alphabetic baseline
computed from their respective metrics._

## How it works

gpui (like CSS) vertically centers a font's `ascent + descent` box inside the
line height and places the baseline at:

```text
baseline_from_top = (line_height − ascent − descent) / 2 + ascent
```

Given a line height that is a whole number of rhythm units, this library solves
that equation backwards: it hands you the padding or margin that puts the
baseline on the next grid line. Plumber's famous `baseline-ratio` turns out to be
a closed-form function of the same metrics — `(em + descent − ascent) / (2·em)` —
which is why looking it up per font is no longer necessary.

## Installation

> **Pre-release:** `main` currently documents the unreleased `0.2.0` API. The
> latest published crate and [docs.rs API](https://docs.rs/rhythm-gpui/0.1.0/rhythm_gpui/)
> are still `0.1.0`; the dependency below will resolve after `0.2.0` is
> published.

```toml
[dependencies]
rhythm-gpui = "0.2"
```

The crate has two layers behind one package:

- **`gpui` feature** (default) — the gpui integration: metric resolution through
  `TextSystem` (with `RhythmFontSpec` cache keys and the resolved `FontId`),
  `Pixels`-typed spacing, drop caps, the fluid-width `RhythmFrame`, the
  `RhythmStyled` extension, and the configurable `RhythmOverlay` debug grid.
- **`default-features = false`** — only the dependency-free rhythm math
  (`Rhythm`, `FontRhythm`, `RhythmLineMetrics`, `RhythmBlockMetrics`,
  `DropCapRhythm`, `snap`, and height snapping). Any renderer that centers
  `ascent + descent` inside the line height can feed it metrics — gpui is not
  compiled at all.

The MSRV is Rust 1.85 for the math-only build and is checked in CI; with the
default `gpui` feature the effective minimum follows gpui itself, which does
not declare one.

## Quick start

```rust
use gpui::{div, font, px, prelude::*};
use rhythm_gpui::{RhythmGrid, RhythmStyled};

// Given a GPUI context named `cx`:
// 1. Pick a grid unit (the analog of rhythm-sass `$rhythm-size`).
let grid = RhythmGrid::new(px(8.));

// 2. Derive fonts from the grid. Line height is given in whole rhythm units:
//    3 × 8px = 24px. Metrics are resolved from the font file at this size.
let body = grid.font(cx.text_system(), font("Georgia"), px(16.), 3);
let heading = grid.font(cx.text_system(), font("Georgia"), px(28.), 5);

// 3. Spacing comes from the fonts themselves, wherever gpui expects a length.
div()
    .px(grid.spacing(6))
    .pt(heading.baseline_top(5))
    .child(div().rhythm_font(&heading).child("Title"))
    .child(
        div()
            .rhythm_font(&body)
            .mt(heading.baseline_between(&body, 6))
            .child("Body text, aligned to the grid."),
    )
```

`grid.spacing(n)` is the axis-neutral length of `n` rhythm units, for layouts
that reuse the same scale for horizontal padding, gaps, or indents.
`grid.height(n)` remains the vertical name for the same length.

Everything else — the spacing functions, `RhythmFont` resolution and accessors,
the `RhythmDropCap` / `DropCapRhythm` solvers, the `RhythmStyled` extension, the
`rhythm_frame` / `RhythmFrame` media container, the `rhythm_overlay` /
`RhythmOverlay` debug grid, and the gpui-free math layer — is documented in the
checked-out crate's rustdoc (`cargo doc --open`). Published versions remain
available on **[docs.rs](https://docs.rs/rhythm-gpui)**.

## Demo & recipes

```
cargo run --example recipes   # font picker, drop cap, heading anchor toggle,
                              # fluid media pad/crop, mixed-font baseline row,
                              # grid overlay;
                              # downloads Google Fonts on demand
```

The toolbar follows a single Tab / Shift-Tab order; Enter or Space activates
the focused font or toggle without a separate keyboard-only behavior path.

The example is a recipe collection:

- **Page scaffold** — open with `baseline_top`, chain blocks with
  `baseline_between`, close with `baseline_bottom`.
- **Optical heading** — `cap_top` lands the capitals' _ink_, not the baseline,
  on a grid line; the paired `cap_bottom` returns the trimmed space so the
  block still spans whole rows and everything below stays in rhythm — closing
  with `baseline_bottom` instead would not. Flip the example's heading toggle to
  compare the two openings live: with the cap anchor, switching typefaces
  keeps the ink pinned while the baseline wobbles, and vice versa.
- **Drop cap with true wrap-around** — `body.drop_cap(ts, font, 3)`
  solves the cap size and baseline anchor from metrics; `.rhythm_drop_cap(&cap)`
  applies the anchor as a relative inset, because for cap-heavy faces (cap
  height > ascent − descent, e.g. Merriweather) the anchor is a _downward_
  shift, and a margin would stretch the flex row and push everything below off
  the grid. Wrap-around text splitting: `drop_cap_paragraph` in the example.
- **Media on the grid** — a fluid-width image's height is not generally an exact
  number of rhythm rows, so everything after it would drift off the grid.
  `rhythm_frame(grid, ratio)`, where `ratio` is width divided by height, fits it
  back on: the frame fills the parent's width and snaps its height
  (`width / ratio`) up to whole rhythm rows, leaving the sub-unit remainder
  below the content; `.crop()` snaps down instead, clipping under one unit
  evenly between the top and bottom edges without resizing the content to the
  snapped height. Style the child to fill the frame's natural-ratio content box
  — use `.size_full().object_fit(ObjectFit::Cover)` when the image itself has a
  different ratio.
  Toggle pad/crop in the example and resize the window: the mixed-font row
  below stays in rhythm at every width. With a known column width, skip the
  frame: `div().w(w).h(grid.snap_up(w / ratio))`.
- **Mixed fonts on one baseline** — different sizes, families, and scripts
  aligned on one grid-seated alphabetic baseline. CJK fonts publish the same
  metrics, but ideographs are drawn on the em square, so their ink dips
  slightly below the shared line — standard mixed-script behavior.
- **Debug overlay** — chain `.rhythm_debug_overlay(grid, show)` on the page
  container, after its content children so the stripes paint on top (gpui
  paints later siblings over earlier ones), to toggle the grid while
  developing; every other grid row is painted in the classic translucent red
  so you can verify baseline-anchored text lands on them. The same toggle
  accepts a configured overlay in place of the grid:
  `.rhythm_debug_overlay(grid.overlay(gpui::rgba(0x0969da33)), show)` picks
  the grid color, and `.phase(content_offset_y)` on the overlay accepts the
  same signed Y translation used to paint content for renderers that scroll
  without moving a scroll container. A gpui `ScrollHandle`'s negative
  `offset().y` can be passed through directly.
- **Device-pixel snapping** — the functions are exact to float precision
  (no whole-pixel rounding, unlike the Sass version); use `snap` to round a
  final value to whole device pixels. gpui's `Window::line_height()` helper
  rounds to whole logical pixels, while ordinary `StyledText` preserves an
  explicitly specified pixel line height before Taffy snaps layout edges to
  device-pixel boundaries. Keep `grid × line_rhythms` whole when the value may
  flow through the rounded helper; independently, each final fractional layout
  edge can differ from the exact math by up to half a device pixel.

## Custom renderers (direct paint)

```
cargo run --example direct_paint   # shape once, cache WrappedLines,
                                   # paint straight onto the grid
```

Document renderers that shape text themselves skip the element layer: build
`RhythmLineMetrics` from each shaped line's real `ascent()` / `descent()` —
the maxima over its explicit font runs, which is how lines mixing bold,
inline code, CJK, or emoji faces actually shape — lay blocks out with
`RhythmBlockMetrics` (baseline or cap-ink anchors, concrete fragment geometry,
and exact integer row arithmetic), and place every baseline with
`paint_origin_for(target)`. Glyph fallback needs no special handling:
substituted glyphs never grow the line box.

Settle the row budget at catalog-build time. A style's lines can draw on more
faces than its own — bold, inline code, an explicit CJK or emoji face, or
whatever gpui resolves to when a family is missing — and each line shapes to
the maxima over its runs. `RhythmFontSpec::resolve_covering(ts, &others)`
resolves the style's font at a line height that holds any mixture of that
same-size, same-grid set (`RhythmLineMetrics::covering` is the pure form), so
"the line box contains the ink" is true by construction instead of checked
per shaped line. Nothing is shaped to compute it, which is what makes a
block's height a function of its line count — the property virtualization
needs.

For a dynamic, non-virtualized line whose configured height is only a floor,
`line_metrics_at_least` remains the one-step overflow-growing path. The fixed
covering budget is the stronger contract when block height must be known before
shaping.

Virtualization runs on integer rows: `first_rows` / `middle_rows` /
`last_rows` are ordered `i32` cursor transitions that partition `rows(lines)`
across a split block. Accumulate them in an `i64`; `baseline_at_row(cursor)`
turns the rebased visible row into its baseline, and `paint_origin_for` then
locates that fragment's line-box top. The final coordinate is still `f32`, so
rebase near the viewport rather than converting an enormous absolute row. A
virtualizer therefore converts only visible positions instead of summing
`f32` heights over thousands of blocks. The `direct_paint` example deliberately
shows the non-virtualized whole-document path; a production virtualizer owns
the absolute `i64` row and viewport-row origin needed for this rebase. Geometry
also carries `Pixels`-typed `_px` mirrors (including `line_height_px`,
`paint_origin_for_px`, `first_baseline_px`, and `baseline_at_row_px`) so gpui
callers need not convert lengths by hand.

The lifecycle contract: a `RhythmFont` is an immutable resolved value. Within
the crate, only its resolution factories query the `TextSystem`; document
renderers still use `shape_text` when their shaped-line cache is stale. Once
metrics are available, the `Rhythm` / `FontRhythm` / line/block geometry paths
are allocation-free `Copy` math (a counting-allocator test enforces that exact
scope). Key caller-owned resolution caches with `RhythmFontSpec`; the crate
keeps no cache of its own. Register a family before its first resolution — gpui
caches failed font requests, so clearing a caller cache after late registration
cannot repair the miss in the same `TextSystem`. The default gpui layer is
compile-checked on Linux and Windows. Mixed-run and glyph-fallback shaping
semantics are CI-verified against macOS/CoreText (`tests/shaping.rs`); the
DirectWrite and cosmic-text runtime behaviors are not yet verified.

**Tip:** as with rhythm-sass, make every text block occupy a whole number of
rhythm units. Line heights are whole units by construction; close a
baseline-anchored block with `baseline_bottom`, or a cap-anchored block with
the paired `cap_bottom`. Blocks then compose freely without breaking the page
rhythm. `.rhythm_block(&font, top, bottom)` applies the baseline-paired recipe
(font + both paddings) in one call, and `font.cap_span(top, bottom)` hands the
cap pair back as one value, reducing the chance of applying mismatched anchors.

## Development

```
git config core.hooksPath .githooks   # once per clone
```

The `pre-commit` hook runs `rustfmt` over the staged `.rs` files and re-stages
them, so what you commit already satisfies CI's `cargo fmt --check`. Markdown,
YAML and JSON are formatted too when Prettier is on `PATH`.

On macOS, `cargo test` also runs the real-shaping suite (`tests/shaping.rs`),
which briefly opens a window — AppKit shaping needs one — and reads the OFL
Noto fonts bundled under `tests/fonts/`. `cargo bench --bench resolve` tracks
warm font-resolution cost as a local dev tool; there is no ns-level CI gate.

`cargo test --features test-support` additionally runs the headless GPUI layout
regression tests; application code does not need this feature.

## Credits

Ported from [rhythm-sass](https://github.com/p233/rhythm-sass), which builds on
concepts introduced by [Plumber](https://jamonserrano.github.io/plumber-sass/).

## License

[MIT](./LICENSE)
