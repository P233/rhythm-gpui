//! Font-picker demo: switch the reading layout between fonts, downloading
//! Google Fonts on demand and registering them with gpui's text system.
//!
//! Showcases metric-derived rhythm: pick any font and every baseline still
//! lands on the grid, with no per-font `baseline-ratio` lookup. Includes a
//! three-line drop cap, a mixed-font run (serif, mono, CJK) sharing one
//! alphabetic baseline, and a toolbar toggle switching the page opening
//! between a baseline-anchored and a cap-anchored (optical) heading. Fonts
//! are fetched as TTF via the css2 API (non-browser user agents are served
//! `truetype` sources) on the background executor.

use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Read;
use std::time::Duration;

use gpui::{
    div, font, prelude::*, px, rgb, size, App, Application, Bounds, ClickEvent, Context, FontStyle,
    FontWeight, Pixels, TextRun, TextSystem, Window, WindowBounds, WindowOptions,
};
use rhythm_gpui::{RhythmDropCap, RhythmFont, RhythmGrid, RhythmStyled};

#[derive(Clone, Copy, PartialEq)]
enum Source {
    Local,
    Google,
}

struct FontChoice {
    family: &'static str,
    source: Source,
}

const CHOICES: &[FontChoice] = &[
    FontChoice {
        family: "Georgia",
        source: Source::Local,
    },
    FontChoice {
        family: "Lora",
        source: Source::Google,
    },
    FontChoice {
        family: "Merriweather",
        source: Source::Google,
    },
    FontChoice {
        family: "Playfair Display",
        source: Source::Google,
    },
    FontChoice {
        family: "Source Serif 4",
        source: Source::Google,
    },
    FontChoice {
        family: "EB Garamond",
        source: Source::Google,
    },
    FontChoice {
        family: "Noto Serif",
        source: Source::Google,
    },
];

const DROP_CAP_LETTER: &str = "W";
const DROP_CAP_REST: &str = "hen William Morris founded the Kelmscott Press in 1891, he insisted \
that a page of text should be judged as a whole: the proportions of the margins, the blackness \
of the type, and above all the even rhythm of its lines. A page reads comfortably when the eye \
can drop from line to line by a constant interval, and that interval — not the font, not the \
measure — is what this demo holds fixed. Switch the typeface above and watch the letterforms \
change while every baseline below keeps its appointment with the grid.";

const PARA_2: &str = "The trick is that nothing here is spaced by eye. Each font publishes its \
ascent and descent in the font file itself, and gpui centers that box inside the line height. \
Knowing both numbers, the library computes exactly how far the first baseline sits below the \
top of a paragraph, and hands back the padding or margin that lands it on the next grid line.";

const PARA_3: &str = "The same arithmetic holds across sizes and families. Below, four runs in \
different fonts and sizes — including a monospaced face and CJK ideographs — are laid out as \
separate elements, yet align on one alphabetic baseline computed from their respective metrics. \
CJK fonts publish the same metrics, but ideographs are drawn on the em square rather than seated \
on the line, so their ink dips slightly below it — the standard behavior for mixed scripts:";

/// The text styles of the reading layout, resolved for one family.
struct FontSet {
    heading: RhythmFont,
    body: RhythmFont,
    quote: RhythmFont,
    display: RhythmFont,
    drop_cap: RhythmDropCap,
}

impl FontSet {
    fn resolve(text_system: &TextSystem, family: &'static str, grid: RhythmGrid) -> Self {
        let mut heading_font = font(family);
        heading_font.weight = FontWeight::BOLD;
        let mut quote_font = font(family);
        quote_font.style = FontStyle::Italic;

        let body = RhythmFont::resolve(text_system, font(family), px(16.), 3, grid);

        // Drop cap sunk 3 body lines: size and baseline anchor are solved from
        // the two faces' metrics.
        let mut cap_font = font(family);
        cap_font.weight = FontWeight::BOLD;
        let drop_cap = RhythmDropCap::resolve(text_system, cap_font, &body, 3);

        Self {
            heading: RhythmFont::resolve(text_system, heading_font, px(36.), 6, grid),
            quote: RhythmFont::resolve(text_system, quote_font, px(16.), 3, grid),
            display: RhythmFont::resolve(text_system, font(family), px(22.), 3, grid),
            body,
            drop_cap,
        }
    }
}

struct Demo {
    grid: RhythmGrid,
    selected: usize,
    set: FontSet,
    mono: RhythmFont,
    cjk: RhythmFont,
    loaded: HashSet<&'static str>,
    loading: Option<&'static str>,
    error: Option<String>,
    show_grid: bool,
    cap_anchor: bool,
}

impl Demo {
    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.loading.is_some() || ix == self.selected {
            return;
        }
        let choice = &CHOICES[ix];
        if choice.source == Source::Local || self.loaded.contains(choice.family) {
            self.set = FontSet::resolve(cx.text_system(), choice.family, self.grid);
            self.selected = ix;
            self.error = None;
            cx.notify();
            return;
        }

        let family = choice.family;
        self.loading = Some(family);
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { fetch_google_family(family) })
                .await;
            this.update(cx, |demo, cx| {
                demo.loading = None;
                match fetched
                    .and_then(|data| cx.text_system().add_fonts(data).map_err(|e| e.to_string()))
                    .and_then(|()| {
                        // Registration success does not guarantee the internal
                        // family name matches (some families ship optical-size
                        // variants like "Family 24pt"); resolving a missing name
                        // would silently fall back to another font.
                        let names = cx.text_system().all_font_names();
                        if names.iter().any(|name| name == family) {
                            Ok(())
                        } else {
                            Err("downloaded fonts registered under a different family name".into())
                        }
                    }) {
                    Ok(()) => {
                        demo.loaded.insert(family);
                        demo.set = FontSet::resolve(cx.text_system(), family, demo.grid);
                        demo.selected = ix;
                    }
                    Err(error) => demo.error = Some(format!("{family}: {error}")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_grid(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.show_grid = !self.show_grid;
        cx.notify();
    }

    fn toggle_anchor(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.cap_anchor = !self.cap_anchor;
        cx.notify();
    }

    // Labels never change while loading (a text change would resize the button
    // and shift the row); progress is shown via opacity and the status line.
    fn font_button(&self, ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let choice = &CHOICES[ix];
        let is_selected = ix == self.selected;
        let is_loading = self.loading == Some(choice.family);
        div()
            .id(ix)
            .px_2()
            .py_0p5()
            .rounded_md()
            .cursor_pointer()
            .map(|d| {
                if is_selected {
                    d.bg(rgb(0x24292f)).text_color(gpui::white())
                } else {
                    d.hover(|s| s.bg(rgb(0xe6e8eb)))
                }
            })
            .when(is_loading, |d| d.opacity(0.55))
            .on_click(cx.listener(move |this, _, _, cx| this.select(ix, cx)))
            .child(choice.family)
    }

    fn status_line(&self) -> (String, u32) {
        if let Some(family) = self.loading {
            return (
                format!("Downloading {family} from fonts.gstatic.com …"),
                0x57606a,
            );
        }
        if let Some(error) = &self.error {
            return (error.clone(), 0xcf222e);
        }
        let body = self.set.body.metrics();
        (
            format!(
                "{} @ {}px — ascent {:.2}, descent {:.2}, implied baseline-ratio {:.4} (derived from the font file)",
                CHOICES[self.selected].family,
                body.font_size(),
                body.ascent(),
                body.descent(),
                body.baseline_ratio(),
            ),
            0x57606a,
        )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let grid = self.grid;
        let (status, status_color) = self.status_line();
        div()
            .flex_none()
            // Height follows content: in a narrow window the button row wraps
            // to several lines, and a fixed height would overflow top and
            // bottom. The toolbar sits above the grid origin (the scrolled
            // wrapper's top edge), so its height is free to vary.
            .py(grid.height(2))
            .px(grid.height(6))
            .flex()
            .flex_col()
            .gap_1()
            .bg(rgb(0xf6f8fa))
            .border_b_1()
            .border_color(rgb(0xd0d7de))
            .text_size(px(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .children((0..CHOICES.len()).map(|ix| self.font_button(ix, cx)))
                    .child(
                        div()
                            .id("grid-toggle")
                            .ml_2()
                            .w(px(72.))
                            .py_0p5()
                            .rounded_md()
                            .cursor_pointer()
                            .border_1()
                            .border_color(rgb(0xd0d7de))
                            .hover(|s| s.bg(rgb(0xe6e8eb)))
                            .flex()
                            .justify_center()
                            .on_click(cx.listener(Self::toggle_grid))
                            .child(if self.show_grid {
                                "Grid: on"
                            } else {
                                "Grid: off"
                            }),
                    )
                    .child(
                        div()
                            .id("anchor-toggle")
                            .ml_2()
                            .w(px(124.))
                            .py_0p5()
                            .rounded_md()
                            .cursor_pointer()
                            .border_1()
                            .border_color(rgb(0xd0d7de))
                            .hover(|s| s.bg(rgb(0xe6e8eb)))
                            .flex()
                            .justify_center()
                            .on_click(cx.listener(Self::toggle_anchor))
                            .child(if self.cap_anchor {
                                "Heading: cap top"
                            } else {
                                "Heading: baseline"
                            }),
                    ),
            )
            .child(
                div()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_color(rgb(status_color))
                    .child(status),
            )
    }

    /// A real wrap-around drop cap: the text is pre-shaped at the exact width
    /// it will occupy beside the cap, split at the start of the fourth visual
    /// line, and the remainder continues full-width below the cap. Line boxes
    /// are whole rhythm units, so the continuation stays on the grid with no
    /// extra spacing.
    fn drop_cap_paragraph(&self, window: &Window, mt: Pixels) -> impl IntoElement {
        let grid = self.grid;
        let set = &self.set;
        let cap_gap = px(12.);
        let text_run = |len: usize, font: &RhythmFont| TextRun {
            len,
            font: font.font().clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let cap_width = window
            .text_system()
            .shape_line(
                DROP_CAP_LETTER.into(),
                set.drop_cap.font().font_size(),
                &[text_run(DROP_CAP_LETTER.len(), set.drop_cap.font())],
                None,
            )
            .width;

        // Same width the flex_1 text child will get: viewport minus the
        // container's horizontal padding, the cap, and the gap.
        let container_width = window.viewport_size().width - grid.height(6) * 2.;
        let wrap_width = (container_width - cap_width - cap_gap).max(px(48.));

        let split = window
            .text_system()
            .shape_text(
                DROP_CAP_REST.into(),
                set.body.font_size(),
                &[text_run(DROP_CAP_REST.len(), &set.body)],
                Some(wrap_width),
                None,
            )
            .ok()
            .and_then(|lines| {
                let line = lines.first()?;
                // The boundary glyph is the first glyph of the wrapped line, so
                // its byte index is where the fourth visual line starts.
                let boundary = line.wrap_boundaries.get(2)?;
                let glyph = &line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix];
                Some(glyph.index)
            })
            .unwrap_or(DROP_CAP_REST.len());
        let (beside_cap, below_cap) = DROP_CAP_REST.split_at(split);

        div()
            .mt(mt)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(cap_gap)
                    .child(div().rhythm_drop_cap(&set.drop_cap).child(DROP_CAP_LETTER))
                    .child(
                        // min_w_0 lets the text shrink below its no-wrap width
                        // so it actually wraps inside the flex row.
                        div()
                            .flex_1()
                            .min_w_0()
                            .rhythm_font(&set.body)
                            .child(beside_cap.to_string()),
                    ),
            )
            .child(
                div()
                    .rhythm_font(&set.body)
                    .child(below_cap.trim_start().to_string()),
            )
    }

    /// Four runs in different fonts and sizes on one shared alphabetic
    /// baseline: pad each span down by the difference between the tallest
    /// `baseline_above` and its own, then place the row so that shared
    /// baseline sits on a grid line. CJK ideographs are drawn on the em square
    /// rather than seated on the baseline (PingFang SC ink reaches ~0.1 em
    /// below it), so their ink dips under the shared line while the metric
    /// alignment stays exact — the same convention CSS inline layout uses.
    fn mixed_font_row(&self) -> impl IntoElement {
        let grid = self.grid;
        let spans: [(&RhythmFont, &'static str, u32); 4] = [
            (&self.set.display, "Display 22", 0x24292f),
            (&self.set.body, "body 16", 0x24292f),
            (&self.mono, "mono_13()", 0x0550ae),
            (&self.cjk, "汉字混排", 0x953800),
        ];
        let max_above = spans
            .iter()
            .map(|(f, ..)| f32::from(f.baseline_above()))
            .fold(0., f32::max);
        div()
            .flex()
            .items_start()
            .gap(grid.height(3))
            .mt(grid.baseline_bottom(&self.set.body, 6) - px(max_above))
            .children(spans.map(|(span_font, text, color)| {
                div()
                    .rhythm_font(span_font)
                    .pt(px(max_above - f32::from(span_font.baseline_above())))
                    .text_color(rgb(color))
                    .whitespace_nowrap()
                    .child(text)
            }))
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let grid = self.grid;
        let family = CHOICES[self.selected].family;
        // Two page openings, switched from the toolbar. Both put the drop-cap
        // paragraph's first baseline 13 units below the page top, so toggling
        // nudges only the heading, never the page below:
        // - baseline: the heading's baseline sits on the 7th grid line, and
        //   switching fonts keeps that baseline fixed while the ink top wobbles.
        // - cap top: the capitals' ink sits on the 4th grid line — switching
        //   fonts keeps the ink fixed while the baseline wobbles — and
        //   cap_bottom returns the trim so the block still spans whole rows.
        //   Faces without usable cap metrics fall back to the baseline opening.
        let cap_anchors = if self.cap_anchor {
            grid.cap_top(&self.set.heading, 4)
                .zip(grid.cap_bottom(&self.set.heading, 0))
        } else {
            None
        };
        let (heading_pt, heading_pb, para_mt) = match cap_anchors {
            Some((pt, pb)) => (pt, pb, grid.baseline_top(&self.set.body, 3)),
            None => (
                grid.baseline_top(&self.set.heading, 7),
                px(0.),
                grid.baseline_between(&self.set.heading, &self.set.body, 6),
            ),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::white())
            .text_color(rgb(0x24292f))
            .child(self.toolbar(cx))
            .child(
                div().id("viewport").flex_1().overflow_y_scroll().child(
                    // The wrapper's top edge is the rhythm grid origin; the
                    // overlay lives inside it so the grid scrolls with the text.
                    div()
                        .min_h_full()
                        .child(
                            div()
                                .px(grid.height(6))
                                .pb(grid.baseline_bottom(&self.set.body, 3))
                                .child(
                                    div()
                                        .rhythm_font(&self.set.heading)
                                        .pt(heading_pt)
                                        .pb(heading_pb)
                                        .child(family),
                                )
                                .child(self.drop_cap_paragraph(window, para_mt))
                                .child(
                                    div()
                                        .rhythm_font(&self.set.body)
                                        .mt(grid.baseline_between(
                                            &self.set.body,
                                            &self.set.body,
                                            6,
                                        ))
                                        .child(PARA_2),
                                )
                                .child(
                                    div()
                                        .rhythm_font(&self.set.quote)
                                        .text_color(rgb(0x57606a))
                                        .mt(grid.baseline_between(
                                            &self.set.body,
                                            &self.set.quote,
                                            6,
                                        ))
                                        .pl(grid.height(3))
                                        .border_l(px(2.))
                                        .border_color(rgb(0xd0d7de))
                                        .child(
                                            "Google Fonts are downloaded as TTF on demand \
                                                 and registered with the text system; no \
                                                 per-font baseline-ratio table is needed.",
                                        ),
                                )
                                .child(
                                    div()
                                        .rhythm_font(&self.set.body)
                                        .mt(grid.baseline_between(
                                            &self.set.quote,
                                            &self.set.body,
                                            6,
                                        ))
                                        .child(PARA_3),
                                )
                                .child(self.mixed_font_row()),
                        )
                        .rhythm_debug_overlay(grid, self.show_grid),
                ),
            )
    }
}

/// Download a Google Fonts family (regular, bold, italic) as TTF bytes.
/// Blocking; run on the background executor.
fn fetch_google_family(family: &str) -> Result<Vec<Cow<'static, [u8]>>, String> {
    let agent = ureq::builder().timeout(Duration::from_secs(30)).build();
    let css_url = format!(
        "https://fonts.googleapis.com/css2?family={}:ital,wght@0,400;0,700;1,400",
        family.replace(' ', "+")
    );
    let css = agent
        .get(&css_url)
        .call()
        .map_err(|e| format!("failed to fetch font CSS: {e}"))?
        .into_string()
        .map_err(|e| format!("failed to read font CSS: {e}"))?;

    let urls = extract_ttf_urls(&css);
    if urls.is_empty() {
        return Err("no TTF sources in css2 response".into());
    }

    let mut fonts = Vec::new();
    for url in urls {
        let mut bytes = Vec::new();
        agent
            .get(&url)
            .call()
            .map_err(|e| format!("failed to fetch font file: {e}"))?
            .into_reader()
            .take(20 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("failed to read font file: {e}"))?;
        fonts.push(Cow::Owned(bytes));
    }
    Ok(fonts)
}

fn extract_ttf_urls(css: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for chunk in css.split("url(").skip(1) {
        if let Some(end) = chunk.find(')') {
            let url = chunk[..end].trim_matches(['\'', '"']).to_string();
            if url.ends_with(".ttf") && !urls.contains(&url) {
                urls.push(url);
            }
        }
    }
    urls
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let grid = RhythmGrid::new(px(8.));
        let text_system = cx.text_system();
        let demo = Demo {
            grid,
            selected: 0,
            set: FontSet::resolve(text_system, CHOICES[0].family, grid),
            mono: RhythmFont::resolve(text_system, font("Menlo"), px(13.), 3, grid),
            cjk: RhythmFont::resolve(text_system, font("PingFang SC"), px(16.), 3, grid),
            loaded: HashSet::new(),
            loading: None,
            error: None,
            show_grid: true,
            cap_anchor: false,
        };

        let bounds = Bounds::centered(None, size(px(840.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| demo),
        )
        .unwrap();
        cx.activate(true);
    });
}
