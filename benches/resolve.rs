//! Warm font-resolution cost: gpui's request-cache lookup plus the metric
//! reads performed by `RhythmFont::resolve`. Pure geometry is covered by the
//! structural zero-allocation test instead of ns-level thresholds, which sit
//! below measurement noise for f32 math.
//!
//! Run locally with `cargo bench --bench resolve`; the numbers are a dev
//! tool, not a CI gate. `Application::headless` provides the real platform
//! text system without a window.

#[cfg(target_os = "macos")]
fn main() {
    use criterion::Criterion;
    use gpui::{font, px, Application};
    use rhythm_gpui::{RhythmFont, RhythmGrid};

    Application::headless().run(|cx: &mut gpui::App| {
        let grid = RhythmGrid::new(px(8.));
        let text_system = cx.text_system().clone();
        let mut criterion = Criterion::default().configure_from_args();

        // Prime gpui's Font → FontId cache, then measure its hit plus the four
        // metric reads. A unique missing family per iteration is deliberately
        // not benchmarked: gpui retains failed request keys, so that would grow
        // process state across samples instead of measuring a stable cold path.
        let serif = font("Georgia");
        let _ = RhythmFont::resolve(&text_system, serif.clone(), px(16.), 3, grid);
        criterion.bench_function("resolve_warm", |b| {
            b.iter(|| RhythmFont::resolve(&text_system, serif.clone(), px(16.), 3, grid))
        });

        criterion.final_summary();
        cx.quit();
    });
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("resolve bench: verified on macOS only");
}
