//! Direct-paint renderer: a custom Element that shapes once, caches the
//! `WrappedLine`s, and paints them straight onto the rhythm grid — the
//! low-level path a document renderer takes instead of building an element
//! tree per block.
//!
//! The recipe, per the numbered comments below:
//!
//! 1. Resolve a `FontSet` once at startup — font-metric resolution happens
//!    only here.
//! 2. Use the window's `TextSystem` to shape paragraphs when the wrap width
//!    changes, and cache the `WrappedLine`s in the view.
//! 3. Build `RhythmLineMetrics` from each line's *shaped* `ascent()` /
//!    `descent()` — for mixed explicit runs these are the maxima over the
//!    runs, which is where the baseline actually goes.
//! 4. Place blocks with `RhythmBlockMetrics` and each line with
//!    `paint_origin_for`, so every baseline lands on the grid.
//! 5. Paint the cached lines directly; wrapped continuation lines advance by
//!    the same whole-row line height and stay on the grid for free.
//!
//! Two paragraphs contain the same CJK-and-emoji text: one styles them as
//! explicit font runs (they enter the line's shaped maxima), one leaves them
//! to glyph fallback (they borrow the primary font's baseline and the line
//! box does not change). Toggle nothing — the overlay shows both behaviors
//! coexisting on one grid.

use std::sync::Arc;

use gpui::{
    div, font, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, Element,
    ElementId, Entity, Font, FontWeight, GlobalElementId, InspectorElementId, LayoutId, Pixels,
    Render, Style, TextAlign, TextRun, TextSystem, Window, WindowBounds, WindowOptions,
    WrappedLine,
};
use rhythm_gpui::{RhythmBlockMetrics, RhythmFont, RhythmGrid, RhythmLineMetrics, RhythmStyled};

/// 1. The text styles, resolved once at startup. Placement below uses only
///    stored metrics; shaping remains an explicit `TextSystem` cache-miss path.
struct FontSet {
    heading: RhythmFont,
    body: RhythmFont,
    bold: RhythmFont,
    mono: RhythmFont,
    cjk: RhythmFont,
    emoji: RhythmFont,
}

impl FontSet {
    fn resolve(text_system: &TextSystem, grid: RhythmGrid) -> Self {
        let mut heading = font("Georgia");
        heading.weight = FontWeight::BOLD;
        let mut bold = font("Georgia");
        bold.weight = FontWeight::BOLD;
        Self {
            heading: grid.font(text_system, heading, px(28.), 5),
            body: grid.font(text_system, font("Georgia"), px(16.), 3),
            bold: grid.font(text_system, bold, px(16.), 3),
            mono: grid.font(text_system, font("Menlo"), px(16.), 3),
            cjk: grid.font(text_system, font("PingFang SC"), px(16.), 3),
            emoji: grid.font(text_system, font("Apple Color Emoji"), px(16.), 3),
        }
    }
}

/// One styled span of a paragraph: its text, font, and color.
struct Span(&'static str, Font, u32);

/// A paragraph to shape: spans, the font size to shape at, and the block's
/// opening/closing rhythm counts.
struct Paragraph {
    spans: Vec<Span>,
    font_size: Pixels,
    top: i32,
    bottom: i32,
}

/// A shaped, positioned block, cached until the wrap width changes.
struct CachedBlock {
    line: WrappedLine,
    metrics: RhythmLineMetrics,
    block: RhythmBlockMetrics,
    /// Block top relative to the document origin, in exact grid rows.
    top_row: i32,
}

struct DocLayout {
    grid: RhythmGrid,
    width: Pixels,
    blocks: Vec<CachedBlock>,
}

struct DirectPaint {
    grid: RhythmGrid,
    set: FontSet,
    cache: Option<Arc<DocLayout>>,
}

const MARGIN: f32 = 48.;

impl DirectPaint {
    fn paragraphs(&self) -> Vec<Paragraph> {
        let set = &self.set;
        let ink = 0x24292f;
        let code = 0x0550ae;
        let script = 0x953800;
        vec![
            Paragraph {
                spans: vec![Span("Direct paint", set.heading.font().clone(), ink)],
                font_size: set.heading.font_size(),
                top: 6,
                bottom: 0,
            },
            Paragraph {
                spans: vec![Span(
                    "Every line below is a cached WrappedLine painted at a computed origin — \
                     no element tree, no Taffy nodes. Resize the window: paragraphs reshape \
                     only when the wrap width changes, and every baseline keeps its \
                     appointment with the grid.",
                    set.body.font().clone(),
                    ink,
                )],
                font_size: set.body.font_size(),
                top: 6,
                bottom: 0,
            },
            // 3. Explicit mixed runs: bold, inline code, CJK, and emoji faces
            //    enter the shaped maxima, so this line box may be taller than
            //    the body font alone — RhythmLineMetrics reads that from the
            //    shaped result and the baseline still lands on the grid.
            Paragraph {
                spans: vec![
                    Span("Explicit runs: ", set.body.font().clone(), ink),
                    Span("bold weight", set.bold.font().clone(), ink),
                    Span(", ", set.body.font().clone(), ink),
                    Span("inline_code()", set.mono.font().clone(), code),
                    Span(", ", set.body.font().clone(), ink),
                    Span("汉字混排", set.cjk.font().clone(), script),
                    Span(" and ", set.body.font().clone(), ink),
                    Span("😀", set.emoji.font().clone(), ink),
                    Span(" share one shaped baseline.", set.body.font().clone(), ink),
                ],
                font_size: set.body.font_size(),
                top: 6,
                bottom: 0,
            },
            // Glyph fallback: the same scripts in a single body-font run. The
            // substituted glyphs borrow the body baseline and the line box
            // stays exactly the body font's — compare the two paragraphs
            // against the overlay.
            Paragraph {
                spans: vec![Span(
                    "Glyph fallback: the same 汉字混排 and 😀 in one body-font run — the \
                     line box does not grow, because fallback glyphs never enter the \
                     shaped ascent.",
                    set.body.font().clone(),
                    ink,
                )],
                font_size: set.body.font_size(),
                top: 6,
                bottom: 3,
            },
        ]
    }

    /// 2. Shape every paragraph at `width` and lay the blocks out on the
    ///    grid. Called only when the cached width goes stale.
    fn layout(&self, width: Pixels, window: &Window) -> DocLayout {
        let grid = self.grid;
        let wrap = px(f32::from(width) - 2. * MARGIN).max(px(64.));
        let mut top_row = 0i32;
        let mut blocks = Vec::new();
        for para in self.paragraphs() {
            let text: String = para.spans.iter().map(|Span(s, ..)| *s).collect();
            let runs: Vec<TextRun> = para
                .spans
                .iter()
                .map(|Span(s, span_font, color)| TextRun {
                    len: s.len(),
                    font: span_font.clone(),
                    color: rgb(*color).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                })
                .collect();
            let lines = window
                .text_system()
                .shape_text(text.into(), para.font_size, &runs, Some(wrap), None)
                .expect("shape direct-paint paragraph");
            for line in lines {
                // 3. Shaped maxima → line metrics. The paragraph's style sets
                //    the line height as a floor; `at_least` grows the box when
                //    an explicit run's ink would not fit.
                let base = if para.font_size == px(28.) { 5 } else { 3 };
                let metrics = grid.line_metrics_at_least(line.ascent(), line.descent(), base);
                let block = RhythmBlockMetrics::new(metrics, para.top, para.bottom);
                let visual_lines = line.wrap_boundaries.len() as u32 + 1;
                // Exact integer rows, never accumulated f32 heights. A
                // virtualizer that splits a block across pages advances its
                // cursor with first_rows / middle_rows / last_rows instead;
                // they partition this same count.
                let block_rows = block.rows(visual_lines);
                blocks.push(CachedBlock {
                    line,
                    metrics,
                    block,
                    top_row,
                });
                top_row = top_row
                    .checked_add(block_rows)
                    .expect("direct-paint document exceeds i32 grid rows");
            }
        }
        DocLayout {
            grid,
            width,
            blocks,
        }
    }
}

/// The custom element: prepaint revalidates the cache, paint replays it.
struct DocumentElement {
    doc: Entity<DirectPaint>,
}

impl IntoElement for DocumentElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for DocumentElement {
    type RequestLayoutState = ();
    type PrepaintState = Arc<DocLayout>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // 2. The cache lives in the view and is rebuilt only when the wrap
        //    width changes; this is the whole "shape once" contract.
        let stale = self
            .doc
            .read(cx)
            .cache
            .as_ref()
            .is_none_or(|c| c.width != bounds.size.width);
        if stale {
            let layout = Arc::new(self.doc.read(cx).layout(bounds.size.width, window));
            self.doc.update(cx, |doc, _| doc.cache = Some(layout));
        }
        self.doc.read(cx).cache.clone().expect("cache just filled")
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for cached in &layout.blocks {
            // 4. Target the first baseline, derive the paint origin, and let
            //    the whole-row line height carry every wrapped line after it.
            let target = layout.grid.height(cached.top_row) + cached.block.first_baseline_px();
            let origin_y = cached.metrics.paint_origin_for_px(target);
            let origin = point(bounds.origin.x + px(MARGIN), bounds.origin.y + origin_y);
            // 5. Paint the cached shaped line — every visual line advances by
            //    the same whole-row line height.
            cached
                .line
                .paint(
                    origin,
                    cached.metrics.line_height_px(),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .expect("paint cached direct-paint line");
        }
    }
}

impl Render for DirectPaint {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(gpui::white())
            .child(DocumentElement { doc: cx.entity() })
            .rhythm_debug_overlay(self.grid, true)
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let grid = RhythmGrid::new(px(8.));
        let view = DirectPaint {
            grid,
            set: FontSet::resolve(cx.text_system(), grid),
            cache: None,
        };
        let bounds = Bounds::centered(None, size(px(760.), px(560.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| view),
        )
        .unwrap();
        cx.activate(true);
    });
}
