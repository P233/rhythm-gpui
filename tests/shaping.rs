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
    use rhythm_gpui::{
        IcfMeasurementError, RhythmFont, RhythmFontSpec, RhythmGrid, RhythmLineMetrics,
    };

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

    /// Group 3: on CoreText, glyphs substituted by font fallback borrow the
    /// primary font's baseline and never enter the line's ascent/descent — the
    /// reported metrics of a single explicit run stay the primary font's,
    /// whatever scripts the text contains. This does not inspect the fallback
    /// glyphs' typographic or raster ink bounds.
    fn glyph_fallback_keeps_primary_line_metrics(window: &Window) {
        let serif = font("Noto Serif");
        let line = shape(window, &[("abc 汉字 😀 def", &serif)]);
        let (ascent, descent) = font_extents(window, &serif);
        assert_close(line.ascent().into(), ascent, "fallback line ascent");
        assert_close(line.descent().into(), descent, "fallback line descent");
    }

    /// Group 4: a reported ascent/descent envelope taller than the chosen line
    /// box reports overflow with negative half-leading, `min_line_rhythms` is
    /// the smallest fitting count, and baseline placement stays exact while
    /// overflowing.
    fn overflow(window: &Window) {
        let grid = RhythmGrid::new(px(8.));
        let serif = font("Noto Serif");
        let emoji = font("Apple Color Emoji");
        let line = shape(window, &[("hi ", &serif), ("😀", &emoji)]);
        let envelope = f32::from(line.ascent()) + f32::from(line.descent());

        let min = grid
            .line_metrics(line.ascent(), line.descent(), 1)
            .min_line_rhythms();
        assert!(min >= 2, "16px mixed metrics span multiple 8px rows");
        assert!(
            f32::from(grid.size()) * min as f32 + TOLERANCE >= envelope,
            "min_line_rhythms must contain the metric envelope"
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

    /// Group 5: a line height resolved to cover a known set of faces holds
    /// every mixture those faces really shape to. The count is fixed before
    /// anything is shaped — the property `min_line_rhythms` can only check
    /// after the fact.
    fn covering_budget(window: &Window) {
        let grid = RhythmGrid::new(px(8.));
        let serif = font("Noto Serif");
        let mut bold = font("Noto Serif");
        bold.weight = FontWeight::BOLD;
        let mono = font("Noto Sans Mono");
        let emoji = font("Apple Color Emoji");
        let tall = font("Zapfino");

        let spec = |face: &Font| RhythmFontSpec::new(face.clone(), px(FONT_SIZE), 3, grid);
        let body = spec(&serif).resolve_covering(
            window.text_system(),
            &[spec(&bold), spec(&mono), spec(&emoji), spec(&tall)],
        );
        let budget = body.metrics().line_rhythms();

        // Zapfino's ~1.8em ascent does not fit the three 8px rows the style
        // asked for, so the covering height really did grow past the request.
        assert!(
            budget > 3,
            "a face taller than the requested line box must raise the budget, got {budget}"
        );
        // Covering never inflates a set that already fits: without the
        // display face, the style keeps the three rows it asked for.
        let fitting = spec(&serif)
            .resolve_covering(window.text_system(), &[spec(&bold), spec(&mono)])
            .metrics()
            .line_rhythms();
        assert_eq!(fitting, 3, "a set that fits must keep the requested height");

        // A gpui shaped line has one font size and one rhythm grid. Reject an
        // incompatible catalog before resolving any of its fonts, rather than
        // measuring a face at a size the line will never use.
        let wrong_size = RhythmFontSpec::new(bold.clone(), px(FONT_SIZE / 2.0), 3, grid);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            spec(&serif).resolve_covering(window.text_system(), &[wrong_size]);
        }))
        .is_err());
        let wrong_grid =
            RhythmFontSpec::new(bold.clone(), px(FONT_SIZE), 3, RhythmGrid::new(px(10.0)));
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            spec(&serif).resolve_covering(window.text_system(), &[wrong_grid]);
        }))
        .is_err());

        // Only the height grew: single-style lines still place from the
        // primary face's own metrics.
        let plain = grid.font(window.text_system(), serif.clone(), px(FONT_SIZE), budget);
        assert_close(
            body.line_metrics().baseline_above(),
            plain.line_metrics().baseline_above(),
            "covering keeps the primary baseline",
        );
        // And it stays a reproducible cache identity: the spec carries the
        // covering count and resolves back to the same metrics.
        let spec_back = body.spec().expect("resolved through the text system");
        assert_eq!(spec_back.line_rhythms(), budget);
        assert_eq!(
            spec_back.resolve(window.text_system()).metrics(),
            body.metrics()
        );

        // Every real mixture of the covered faces fits that one budget, so a
        // renderer can size blocks from line counts before shaping.
        let cases: [Vec<(&str, &Font)>; 4] = [
            vec![("plain body text", &serif)],
            vec![("body ", &serif), ("bold ", &bold), ("mono()", &mono)],
            vec![("hi ", &serif), ("😀", &emoji)],
            vec![("plain ", &serif), ("flourish", &tall)],
        ];
        for parts in cases {
            let line = shape(window, &parts);
            let metrics = grid.line_metrics(line.ascent(), line.descent(), budget);
            assert!(
                !metrics.overflows_line_box(),
                "covered mixture overflowed the budget of {budget} rows"
            );
            // The budget was settled before shaping: no shaped mixture needs
            // a taller box than the covering count.
            assert!(metrics.min_line_rhythms() <= budget);
            assert_eq!(
                grid.line_metrics_at_least(line.ascent(), line.descent(), budget),
                metrics
            );
            assert_paint_formula(&metrics);
        }
    }

    /// Group 6: can the ideographic character face be *measured* through
    /// gpui, instead of being hand-copied from the font's `BASE` table?
    ///
    /// `TextSystem::typographic_bounds` reports per-glyph ink, so the answer
    /// decides whether ICF anchoring needs a caller-supplied constant at all.
    /// Ground truth from PingFang SC's own tables (fonttools, em units above
    /// the baseline): `icft` +0.822, `icfb` −0.102, with 字 +0.825/−0.096,
    /// 永 +0.821/−0.101, 語 +0.823/−0.102 — and 国 only +0.771, which is why
    /// a probe set needs several full-frame glyphs rather than one.
    fn ideographic_ink(cx: &mut App) {
        let text_system = cx.text_system();
        let size = px(FONT_SIZE);
        let grid = RhythmGrid::new(px(8.));

        // CoreText maps glyph 0 to `None` before asking for typographic bounds.
        // The bundled Latin-only face makes that backend contract independent
        // of system-font availability: every CJK probe must fail lookup, which
        // the public measurement boundary preserves as `NoProbeBounds`.
        let latin = grid.font(text_system, font("Noto Serif"), size, 3);
        assert_eq!(
            latin.measure_icf(text_system, "字永語国").unwrap_err(),
            IcfMeasurementError::NoProbeBounds
        );

        if !text_system
            .all_font_names()
            .iter()
            .any(|name| name == "PingFang SC")
        {
            println!("  (ideographic ink group skipped: PingFang SC unavailable)");
            return;
        }
        let font_id = text_system.resolve_font(&font("PingFang SC"));

        let mut ink_top = f32::MIN;
        let mut ink_bottom = f32::MAX;
        for ch in ['字', '永', '語', '国'] {
            let bounds = text_system
                .typographic_bounds(font_id, size, ch)
                .unwrap_or_else(|e| panic!("typographic bounds for {ch}: {e}"));
            // gpui preserves CoreText's glyph-space rectangle: origin.y is
            // the ink bottom and origin.y + height is its top.
            let bottom = f32::from(bounds.origin.y);
            let top = f32::from(bounds.origin.y + bounds.size.height);
            println!(
                "  {ch}: top {:+.4} em, bottom {:+.4} em",
                top / FONT_SIZE,
                bottom / FONT_SIZE
            );
            ink_top = ink_top.max(top);
            ink_bottom = ink_bottom.min(bottom);
        }

        let icft = ink_top / FONT_SIZE;
        let icfb = ink_bottom / FONT_SIZE;
        assert!(
            (icft - 0.822).abs() < 0.01,
            "measured ICF top {icft:+.4} em should match PingFang's icft +0.822"
        );
        assert!(
            (icfb + 0.102).abs() < 0.01,
            "measured ICF bottom {icfb:+.4} em should match PingFang's icfb −0.102"
        );

        // The public path must agree with the raw probe, and anchor on it.
        let heading = grid.font(text_system, font("PingFang SC"), size, 3);
        let anchor = heading
            .measure_icf(text_system, "字永語国")
            .expect("measuring a resolved face yields an ICF anchor");
        assert_eq!(
            anchor.font().spec(),
            heading.spec(),
            "measurement must preserve the resolved font identity"
        );
        let trim = anchor.trim_top();
        assert_close(
            f32::from(anchor.font().baseline_above() - trim),
            ink_top,
            "measure_icf should store the tallest probe ink in the anchor",
        );
        let (pt, pb) = anchor.span(3, 0);
        assert_close(
            f32::from(pt + trim),
            f32::from(grid.height(3)),
            "the ICF span must land the measured ink on the grid line",
        );
        let rows = f32::from(pt + anchor.font().line_height() + pb) / 8.0;
        assert_close(rows, rows.round(), "the ICF pair must span whole rows");

        // Measurement does not mutate the resolved font or a previous anchor.
        // A failed independent attempt reports the failure at this boundary.
        let unavailable = heading.measure_icf(text_system, "").unwrap_err();
        assert_eq!(unavailable, IcfMeasurementError::EmptyProbes);
        assert_eq!(anchor.span(3, 0), (pt, pb));

        let synthetic = RhythmFont::from_baseline_ratio(font("PingFang SC"), size, 3, 0.2, grid);
        assert_eq!(
            synthetic.measure_icf(text_system, "字").unwrap_err(),
            IcfMeasurementError::UnresolvedFont
        );

        // A Japanese setting probes kana too: their marks can rise above the
        // han envelope, as Latin ascenders rise above cap height. How far they
        // rise belongs to the installed face, and PingFang SC ships versions
        // where they do not rise at all, so this pins the fold rather than the
        // overshoot: the anchor must hold the tallest accepted ink over the
        // whole probe set, whichever script contributes it.
        let mut probe_set_top = ink_top;
        for ch in ['ぱ', 'ポ'] {
            let Ok(bounds) = text_system.typographic_bounds(font_id, size, ch) else {
                println!("  {ch}: no bounds, skipped exactly as measure_icf skips it");
                continue;
            };
            let bottom = f32::from(bounds.origin.y);
            let top = f32::from(bounds.origin.y + bounds.size.height);
            println!(
                "  {ch}: top {:+.4} em, bottom {:+.4} em",
                top / FONT_SIZE,
                bottom / FONT_SIZE
            );
            // measure_icf accepts only ink straddling the alphabetic baseline.
            if bottom < 0.0 && top > 0.0 {
                probe_set_top = probe_set_top.max(top);
            }
        }

        let japanese = grid.font(text_system, font("PingFang SC"), size, 3);
        let with_kana = japanese
            .measure_icf(text_system, "字永語国ぱポ")
            .expect("kana probes resolve");
        assert_close(
            f32::from(with_kana.font().baseline_above() - with_kana.trim_top()),
            probe_set_top,
            "measure_icf must fold the tallest accepted ink over the whole probe set",
        );

        // A face with no BASE table at all still measures: the whole point of
        // preferring measurement to a table lookup.
        if text_system
            .all_font_names()
            .iter()
            .any(|name| name == "SimSong")
        {
            let simsong = grid.font(text_system, font("SimSong"), size, 3);
            simsong
                .measure_icf(text_system, "字永語国")
                .expect("measurement must work without a BASE table");
        }
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
                    glyph_fallback_keeps_primary_line_metrics(window);
                    overflow(window);
                    covering_budget(window);
                    ideographic_ink(cx);
                    cx.new(|_| TestView)
                },
            )
            .expect("open the shaping test window");
            // On macOS quit terminates the process without returning from
            // `run`, so report success before it.
            println!(
                "shaping suite passed: single-run, mixed-run max, glyph fallback, \
                 overflow, covering budget, ideographic ink"
            );
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
