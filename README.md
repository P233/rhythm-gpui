# rhythm-gpui

[![CI](https://github.com/p233/rhythm-gpui/actions/workflows/ci.yml/badge.svg)](https://github.com/p233/rhythm-gpui/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rhythm-gpui.svg)](https://crates.io/crates/rhythm-gpui)
[![docs.rs](https://img.shields.io/docsrs/rhythm-gpui)](https://docs.rs/rhythm-gpui)

Print-inspired vertical rhythm for [gpui](https://www.gpui.rs) — baseline
offsets computed from real font metrics. Ported from
[rhythm-sass](https://github.com/p233/rhythm-sass), it lands every text
baseline exactly on a vertical rhythm grid, giving you pixel-perfect control
over typographic layout in gpui apps.

Unlike the Sass original, **no hand-measured `baseline-ratio` is required**:
metrics are read from the actual font file through gpui's text system.

![The demo with the grid overlay on: a three-line drop cap and four fonts of different sizes and scripts sharing one baseline, every baseline landing on the grid](assets/demo.png)

_`cargo run --example demo` — switch the typeface and every baseline keeps its
appointment with the grid: the drop cap spans three lines, and four runs
(serif, monospace, CJK) share a single alphabetic baseline computed from their
respective metrics._

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

```toml
[dependencies]
rhythm-gpui = "0.1"
```

The crate has two layers behind one package:

- **`gpui` feature** (default) — the gpui integration: metric resolution through
  `TextSystem`, `Pixels`-typed spacing, drop caps, the `RhythmStyled` extension,
  and the debug overlay.
- **`default-features = false`** — only the dependency-free rhythm math
  (`Rhythm`, `FontRhythm`, `DropCapRhythm`, `snap`). Any renderer that centers
  `ascent + descent` inside the line height can feed it metrics — gpui is not
  compiled at all.

macOS needs the Metal toolchain to build gpui:
`xcodebuild -downloadComponent MetalToolchain`.

The MSRV is Rust 1.85 for the math-only build and is checked in CI; with the
default `gpui` feature the effective minimum follows gpui itself, which does
not declare one.

## Quick start

```rust
use gpui::{div, font, px, prelude::*};
use rhythm_gpui::{RhythmFont, RhythmGrid, RhythmStyled};

// 1. Pick a grid unit (the analog of rhythm-sass `$rhythm-size`).
let grid = RhythmGrid::new(px(8.));

// 2. Bind fonts to the grid. Line height is given in whole rhythm units:
//    3 × 8px = 24px. Metrics are resolved from the font file at this size.
let body = RhythmFont::resolve(cx.text_system(), font("Georgia"), px(16.), 3, grid);
let heading = RhythmFont::resolve(cx.text_system(), font("Georgia"), px(28.), 5, grid);

// 3. Use the spacing functions wherever gpui expects a length.
div()
    .pt(grid.baseline_top(&heading, 5))
    .child(div().rhythm_font(&heading).child("Title"))
    .child(
        div()
            .rhythm_font(&body)
            .mt(grid.baseline_between(&heading, &body, 6))
            .child("Body text, aligned to the grid."),
    )
```

Everything else — the spacing functions, `RhythmFont` resolution and accessors,
the `RhythmDropCap` / `DropCapRhythm` solvers, the `RhythmStyled` extension, the
`rhythm_overlay` debug grid, and the gpui-free math layer — is documented on
**[docs.rs](https://docs.rs/rhythm-gpui)**.

## Demo & recipes

```
cargo run --example demo   # font picker, drop cap, heading anchor toggle,
                           # mixed-font baseline row, grid overlay;
                           # downloads Google Fonts on demand
```

The demo doubles as a recipe collection:

- **Page scaffold** — open with `baseline_top`, chain blocks with
  `baseline_between`, close with `baseline_bottom`.
- **Optical heading** — `cap_top` lands the capitals' _ink_, not the baseline,
  on a grid line; the paired `cap_bottom` returns the trimmed space so the
  block still spans whole rows and everything below stays in rhythm — closing
  with `baseline_bottom` instead would not. Flip the demo's heading toggle to
  compare the two openings live: with the cap anchor, switching typefaces
  keeps the ink pinned while the baseline wobbles, and vice versa.
- **Drop cap with true wrap-around** — `RhythmDropCap::resolve(ts, font, &body, 3)`
  solves the cap size and baseline anchor from metrics; `.rhythm_drop_cap(&cap)`
  applies the anchor as a relative inset, because for cap-heavy faces (cap
  height > ascent − descent, e.g. Merriweather) the anchor is a _downward_
  shift, and a margin would stretch the flex row and push everything below off
  the grid. Wrap-around text splitting: `drop_cap_paragraph` in the demo.
- **Mixed fonts on one baseline** — different sizes, families, and scripts
  aligned on one grid-seated alphabetic baseline. CJK fonts publish the same
  metrics, but ideographs are drawn on the em square, so their ink dips
  slightly below the shared line — standard mixed-script behavior.
- **Debug overlay** — chain `.rhythm_debug_overlay(grid, show)` on the page
  container to toggle the stripes while developing; every other grid row is
  painted so you can verify baselines land on them. The underlying
  `rhythm_overlay(grid, color)` element takes a custom color.
- **Device-pixel snapping** — the functions are exact to float precision
  (no whole-pixel rounding, unlike the Sass version); use `snap` to round a
  final value to whole device pixels. gpui rounds text line heights to whole
  logical pixels and snaps Taffy layout edges to device-pixel boundaries. Keep
  `grid × line_rhythms` whole in logical pixels so gpui does not alter the line
  height; each remaining fractional layout edge can differ from the exact math
  by up to half a device pixel.

**Tip:** as with rhythm-sass, make every text block occupy a whole number of
rhythm units. Line heights are whole units by construction; close a
baseline-anchored block with `baseline_bottom`, or a cap-anchored block with
the paired `cap_bottom`. Blocks then compose freely without breaking the page
rhythm.

## Development

```
git config core.hooksPath .githooks   # once per clone
```

The `pre-commit` hook runs `rustfmt` over the staged `.rs` files and re-stages
them, so what you commit already satisfies CI's `cargo fmt --check`. Markdown,
YAML and JSON are formatted too when Prettier is on `PATH`.

## Credits

Ported from [rhythm-sass](https://github.com/p233/rhythm-sass), which builds on
concepts introduced by [Plumber](https://jamonserrano.github.io/plumber-sass/).

## License

[MIT](./LICENSE)
