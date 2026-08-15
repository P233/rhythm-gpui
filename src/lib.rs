//! Vertical rhythm typography for [gpui](https://www.gpui.rs), ported from
//! [rhythm-sass](https://github.com/p233/rhythm-sass).
//!
//! gpui paints text exactly like the model in [`math`](crate::Rhythm): the
//! `ascent + descent` box is centered in the line height and the baseline sits at
//! `(line_height - ascent - descent) / 2 + ascent` (see `TextSystem::baseline_offset`).
//! This crate resolves real font metrics through gpui's text system, so baselines
//! land on the rhythm grid without the manually measured `baseline-ratio` that the
//! original Sass library required.
//!
//! # Feature flags
//!
//! - **`gpui`** (default) — the gpui integration: `RhythmGrid`, `RhythmFont`,
//!   `RhythmDropCap`, the `RhythmStyled` extension trait, and the
//!   `rhythm_overlay` debug grid.
//! - Disable default features to build only the dependency-free rhythm math
//!   ([`Rhythm`], [`FontRhythm`], [`DropCapRhythm`], [`snap`]), usable from any
//!   renderer that centers `ascent + descent` inside the line height:
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
//! wrap-around, and mixed fonts (sizes, families, scripts) sharing one
//! alphabetic baseline.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

mod math;

pub use math::{snap, DropCapRhythm, FontRhythm, Rhythm};

#[cfg(feature = "gpui")]
mod integration;

#[cfg(feature = "gpui")]
pub use integration::{rhythm_overlay, RhythmDropCap, RhythmFont, RhythmGrid, RhythmStyled};
