//! Real-shaping integration tests against gpui's macOS/CoreText backend.
//!
//! `harness = false`: AppKit requires the process main thread, which the
//! libtest harness does not provide, so the suite runs as its own binary. It
//! opens one real window (shaping lives on `WindowTextSystem`, which only a
//! window provides), runs every group against real CoreText shaping, and
//! quits; any failed assertion panics and fails the binary.
//!
//! Scope: the max-run and glyph-fallback semantics asserted here are
//! CoreText behavior. Other gpui text backends are not verified and must not
//! be assumed identical.
//!
//! `TextSystem::baseline_offset` is deliberately *not* used as the oracle:
//! on macOS it computes with the raw OpenType-negative descent while the
//! paint path uses the shaped positive descent, so its result differs from
//! the painted baseline by one descent. The suite instead pins the sign
//! convention itself (see `single_explicit_run`) and checks placement
//! against the paint equation `origin + (line_height − ascent − descent) / 2
//! + ascent` from gpui's `paint_line`.

#[cfg(target_os = "macos")]
mod suite {
    use std::borrow::Cow;
    use std::path::{Path, PathBuf};

    use gpui::{
        font, px, size, App, AppContext, Application, Bounds, Context, Font, FontStyle, FontWeight,
        IntoElement, Render, TextRun, Window, WindowBounds, WindowOptions, WrappedLine,
    };
    use rhythm_gpui::{RhythmGrid, RhythmLineMetrics};

    const FONT_SIZE: f32 = 16.0;
    const TOLERANCE: f32 = 0.02;

    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::Empty
        }
    }

    fn fonts_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts")
    }

    fn load_fonts(dir: &Path) -> Vec<Cow<'static, [u8]>> {
        [
            "NotoSerif-Regular.ttf",
            "NotoSerif-Bold.ttf",
            "NotoSerif-Italic.ttf",
            "NotoSansMono-Regular.ttf",
        ]
        .iter()
        .map(|name| {
            let path = dir.join(name);
            Cow::Owned(std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}")))
        })
        .collect()
    }

    fn assert_close(actual: f32, expected: f32, what: &str) {
        assert!(
            (actual - expected).abs() < TOLERANCE,
            "{what}: expected {expected}, got {actual}"
        );
    }

    fn text_run(len: usize, font: &Font) -> TextRun {
        TextRun {
            len,
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    /// Shape `parts` as one logical line of explicit runs and return it with
    /// the concatenated text's shaped metrics.
    fn shape(window: &Window, parts: &[(&str, &Font)]) -> WrappedLine {
        let text: String = parts.iter().map(|(s, _)| *s).collect();
        let runs: Vec<TextRun> = parts
            .iter()
            .map(|(s, style_font)| text_run(s.len(), style_font))
            .collect();
        let lines = window
            .text_system()
            .shape_text(text.into(), px(FONT_SIZE), &runs, None, None)
            .expect("shape_text");
        assert_eq!(lines.len(), 1, "test text is a single logical line");
        lines[0].clone()
    }

    /// The positive ascent/descent magnitudes gpui's shaper derives for a
    /// font, from the same `TextSystem` the line was shaped by.
    fn font_extents(window: &Window, style_font: &Font) -> (f32, f32) {
        let ts = window.text_system();
        let id = ts.resolve_font(style_font);
        (
            f32::from(ts.ascent(id, px(FONT_SIZE))),
            -f32::from(ts.descent(id, px(FONT_SIZE))),
        )
    }

    /// The painted baseline for a line placed at `paint_origin_for(target)`,
    /// per gpui's `paint_line`: `origin + (line_height − ascent − descent) /
    /// 2 + ascent`. Must land on `target` exactly.
    fn assert_paint_formula(metrics: &RhythmLineMetrics) {
        let target = metrics.grid().height(7);
        let origin = metrics.paint_origin_for(target);
        let painted = origin
            + (metrics.line_height() - metrics.ascent() - metrics.descent()) / 2.0
            + metrics.ascent();
        assert_close(painted, target, "painted baseline");
    }

    /// Group 1: a single explicit run's shaped metrics are exactly the
    /// resolved font's metrics, the sign convention holds, and the resolved
    /// and shaped paths produce one placement.
    fn single_explicit_run(window: &Window) {
        let grid = RhythmGrid::new(px(8.));
        let serif = font("Noto Serif");
        let ts = window.text_system();
        let raw_descent = f32::from(ts.descent(ts.resolve_font(&serif), px(FONT_SIZE)));
        assert!(
            raw_descent < 0.0,
            "FontMetrics descent should be OpenType-negative on macOS; if this \
             flipped upstream, re-check from_platform_metrics normalization \
             and this suite's descent handling"
        );

        let line = shape(window, &[("Hamburgefonstiv 123", &serif)]);
        let (ascent, descent) = font_extents(window, &serif);
        assert_close(line.ascent().into(), ascent, "single-run ascent");
        assert_close(line.descent().into(), descent, "single-run descent");

        let shaped = grid.line_metrics(line.ascent(), line.descent(), 3);
        let resolved = grid
            .font(window.text_system(), serif, px(FONT_SIZE), 3)
            .line_metrics();
        assert_close(
            shaped.baseline_above(),
            resolved.baseline_above(),
            "resolved vs shaped baseline",
        );
        assert_paint_formula(&shaped);
    }

    /// Group 2: a line of mixed explicit runs is shaped with the maximum
    /// ascent/descent over its runs, and the target-baseline formula holds
    /// for the shaped values.
    fn mixed_explicit_runs(window: &Window) {
        let grid = RhythmGrid::new(px(8.));
        let serif = font("Noto Serif");
        let mut bold = font("Noto Serif");
        bold.weight = FontWeight::BOLD;
        let mut italic = font("Noto Serif");
        italic.style = FontStyle::Italic;
        let mono = font("Noto Sans Mono");
        let cjk = font("PingFang SC");
        let emoji = font("Apple Color Emoji");
        // Zapfino's ~1.8em ascent guarantees a case where a secondary run,
        // not the primary font, determines the line box. Noto Serif's own
        // 1.069em ascent tops PingFang SC and even Apple Color Emoji (hhea
        // ascent 0.77em — an explicit emoji run does *not* inflate a
        // same-size line on macOS), so without it every max below would
        // degenerate to the primary font's metrics.
        let tall = font("Zapfino");

        let cases: [(&str, Vec<(&str, &Font)>); 5] = [
            (
                "regular + bold + italic",
                vec![("regular ", &serif), ("bold ", &bold), ("italic", &italic)],
            ),
            (
                "serif + monospace inline code",
                vec![("body ", &serif), ("mono_span()", &mono)],
            ),
            (
                "serif + explicit CJK",
                vec![("serif ", &serif), ("汉字混排", &cjk)],
            ),
            (
                "serif + explicit emoji",
                vec![("hi ", &serif), ("😀", &emoji)],
            ),
            (
                "serif + tall display face",
                vec![("plain ", &serif), ("flourish", &tall)],
            ),
        ];

        for (name, parts) in cases {
            let line = shape(window, &parts);
            let (max_ascent, max_descent) = parts
                .iter()
                .map(|(_, style_font)| font_extents(window, style_font))
                .fold((0.0f32, 0.0f32), |(a, d), (fa, fd)| (a.max(fa), d.max(fd)));
            assert_close(line.ascent().into(), max_ascent, &format!("{name}: ascent"));
            assert_close(
                line.descent().into(),
                max_descent,
                &format!("{name}: descent"),
            );
            assert_paint_formula(&grid.line_metrics(line.ascent(), line.descent(), 4));
        }

        // Sanity that the suite exercises real inflation somewhere: the tall
        // face must raise the line above the serif's own ascent, so the max
        // assertions above are not all degenerate.
        let (serif_ascent, _) = font_extents(window, &serif);
        let (tall_ascent, _) = font_extents(window, &tall);
        assert!(
            tall_ascent > serif_ascent + 1.0,
            "Zapfino should be far taller than Noto Serif \
             ({tall_ascent} vs {serif_ascent}); did resolution fall back?"
        );
    }

    /// Group 3: glyphs substituted by font fallback borrow the primary
    /// font's baseline and never enter the line's ascent/descent — the line
    /// box of a single explicit run stays the primary font's, whatever
    /// scripts the text contains.
    fn glyph_fallback(window: &Window) {
        let serif = font("Noto Serif");
        let line = shape(window, &[("abc 汉字 😀 def", &serif)]);
        let (ascent, descent) = font_extents(window, &serif);
        assert_close(line.ascent().into(), ascent, "fallback line ascent");
        assert_close(line.descent().into(), descent, "fallback line descent");
    }

    /// Group 4: ink taller than the chosen line box reports overflow with a
    /// negative half-leading, `min_line_rhythms` is the smallest fitting
    /// count, and baseline placement stays exact while overflowing.
    fn overflow(window: &Window) {
        let grid = RhythmGrid::new(px(8.));
        let serif = font("Noto Serif");
        let emoji = font("Apple Color Emoji");
        let line = shape(window, &[("hi ", &serif), ("😀", &emoji)]);
        let ink = f32::from(line.ascent()) + f32::from(line.descent());

        let min = grid
            .line_metrics(line.ascent(), line.descent(), 1)
            .min_line_rhythms();
        assert!(min >= 2, "16px mixed ink spans multiple 8px rows");
        assert!(
            f32::from(grid.size()) * min as f32 + TOLERANCE >= ink,
            "min_line_rhythms must contain the ink"
        );

        let small = grid.line_metrics(line.ascent(), line.descent(), min - 1);
        assert!(small.overflows_line_box(), "under-tall box must overflow");
        assert!(
            small.half_leading() < 0.0,
            "overflow means negative half-leading"
        );
        assert_paint_formula(&small);

        let fit = grid.line_metrics(line.ascent(), line.descent(), min);
        assert!(!fit.overflows_line_box(), "min_line_rhythms must fit");
    }

    pub fn run() {
        let dir = fonts_dir();
        if !dir.join("NotoSerif-Regular.ttf").exists() {
            println!(
                "shaping suite skipped: tests/fonts assets are not present \
                 (they are kept out of the published package)"
            );
            return;
        }

        // Never hang CI: the suite normally finishes in seconds.
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(120));
            eprintln!("shaping suite timed out");
            std::process::exit(2);
        });

        Application::new().run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(load_fonts(&dir))
                .expect("register bundled test fonts");
            let names = cx.text_system().all_font_names();
            for family in ["Noto Serif", "Noto Sans Mono"] {
                assert!(
                    names.iter().any(|name| name == family),
                    "bundled {family} did not register under its family name"
                );
            }

            let bounds = Bounds::centered(None, size(px(640.), px(480.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    single_explicit_run(window);
                    mixed_explicit_runs(window);
                    glyph_fallback(window);
                    overflow(window);
                    cx.new(|_| TestView)
                },
            )
            .expect("open the shaping test window");
            // On macOS quit terminates the process without returning from
            // `run`, so report success before it.
            println!("shaping suite passed: single-run, mixed-run max, glyph fallback, overflow");
            cx.quit();
        });
    }
}

#[cfg(target_os = "macos")]
fn main() {
    suite::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("shaping suite skipped: it covers the macOS/CoreText backend only");
}
