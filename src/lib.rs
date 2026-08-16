//! Vertical rhythm typography for [gpui](https://www.gpui.rs), ported from
//! [rhythm-sass](https://github.com/p233/rhythm-sass).
//!
//! gpui's `WrappedLine` paint path follows the model in [`math`](crate::Rhythm):
//! the `ascent + descent` box is centered in the line height and the baseline
//! sits at `(line_height - ascent - descent) / 2 + ascent`.
//! This crate resolves real font metrics through gpui's text system, so baselines
//! land on the rhythm grid without the manually measured `baseline-ratio` that the
//! original Sass library required.
//!
//! # Feature flags
//!
//! - **`gpui`** (default) — the gpui integration: `RhythmGrid`, `RhythmFont`
//!   (with `RhythmFontSpec` cache keys), `RhythmDropCap`, the `RhythmStyled`
//!   extension trait, the `rhythm_frame` media container, and the
//!   `rhythm_overlay` debug grid.
//! - Disable default features to build only the dependency-free rhythm math
//!   ([`Rhythm`], [`FontRhythm`], [`RhythmLineMetrics`], [`RhythmBlockMetrics`],
//!   [`DropCapRhythm`], [`snap`]), usable from any renderer that centers
//!   `ascent + descent` inside the line height:
//!
//!   ```toml
//!   rhythm-gpui = { version = "0.2", default-features = false }
//!   ```
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "gpui")]
//! use gpui::{div, font, px, prelude::*};
//! # #[cfg(feature = "gpui")]
//! use rhythm_gpui::{RhythmGrid, RhythmStyled};
//!
//! # #[cfg(feature = "gpui")]
//! fn body(text_system: &gpui::TextSystem) -> impl IntoElement {
//!     let grid = RhythmGrid::new(px(8.));
//!     let para = grid.font(text_system, font("Georgia"), px(16.), 3);
//!     div()
//!         .rhythm_block(&para, 3, 1)
//!         .child("Aligned to the grid.")
//! }
//! ```
//!
//! The repository's `demo` example doubles as a recipe collection: a page
//! scaffold, baseline- and cap-anchored openings, a drop cap with true
//! wrap-around, fluid-width media padded or cropped to whole rhythm rows, and
//! mixed fonts (sizes, families, scripts) sharing one alphabetic baseline.
//!
//! # Custom renderers
//!
//! Document renderers that shape text themselves and paint cached
//! `WrappedLine`s skip the element layer entirely: build [`RhythmLineMetrics`]
//! from each shaped line's real `ascent()` / `descent()` — the maxima over its
//! explicit font runs, which is how lines mixing bold, inline code, CJK, or
//! emoji faces actually shape — lay blocks out with [`RhythmBlockMetrics`]
//! (first/middle/last fragment heights, plus the integer `first_rows` /
//! `middle_rows` / `last_rows` cursor and `continuation_baseline` for split
//! blocks), and place every baseline with `paint_origin_for`. Under the `gpui`
//! feature both metric types also expose `Pixels`-typed `_px` mirrors, so the
//! paint path never converts by hand. The `direct_paint` example is the
//! complete recipe.
//!
//! Settle each style's line height at catalog-build time with
//! [`RhythmLineMetrics::covering`] — or `RhythmFontSpec::resolve_covering`,
//! which resolves a font at the count covering its whole same-size run set.
//! Because no mixture of those faces can shape past the covering box, the row
//! budget is known before anything is shaped, and a block's height follows
//! from its line count alone: the property virtualization is built on.
//!
//! # Performance contract
//!
//! - [`Rhythm`], [`FontRhythm`], and line/block geometry are small `Copy`
//!   values. Their scalar geometry and spacing methods are O(1), while
//!   [`RhythmLineMetrics::covering`] is O(`metrics.len()`). All remain
//!   allocation-free and lock-free (enforced by a counting-allocator test),
//!   with hot methods `#[inline]` across the crate boundary.
//! - gpui `TextSystem` access is confined to font/spec/drop-cap resolution
//!   factories; geometry and spacing on stored values never query it.
//! - The shaped-line adapter reads a line's already-computed ascent/descent
//!   and never walks glyphs.
//! - `cargo bench --bench resolve` tracks warm font resolution (gpui's request
//!   cache hit plus metric reads). There is deliberately no ns-level CI
//!   threshold: pure `f32` math varies below measurement noise, so the
//!   guarantees above are structural, not benchmarked.
//!
//! # Platform scope
//!
//! The math layer is renderer- and platform-agnostic, and the resolved-font
//! path uses cross-platform gpui API. The mixed-run maximum and
//! glyph-fallback behaviors are verified against macOS/CoreText by the
//! `shaping` integration suite; other gpui text backends (DirectWrite,
//! cosmic-text) are not yet verified and must not be assumed identical.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

mod math;
mod metrics;

pub use math::{snap, DropCapRhythm, FontRhythm, Rhythm};
pub use metrics::{RhythmBlockMetrics, RhythmLineMetrics};

#[cfg(feature = "gpui")]
mod frame;
#[cfg(feature = "gpui")]
mod integration;

#[cfg(feature = "gpui")]
pub use frame::{rhythm_frame, RhythmFrame};
#[cfg(feature = "gpui")]
pub use integration::{
    rhythm_overlay, RhythmDropCap, RhythmFont, RhythmFontSpec, RhythmGrid, RhythmOverlay,
    RhythmStyled,
};
