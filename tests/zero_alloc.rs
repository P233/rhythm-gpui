//! Structural performance contract: hot-path geometry allocates nothing.
//!
//! A counting global allocator replaces ns-level benchmark thresholds — pure
//! f32 math varies below measurement noise, but "no allocation, no lock, no
//! text-system access" is checkable exactly. The single `#[test]` keeps the
//! harness quiet while the counter window is open.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use rhythm_gpui::{FontRhythm, Rhythm, RhythmBlockMetrics, RhythmLineMetrics};

struct CountingAlloc;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[test]
fn hot_path_geometry_allocates_nothing() {
    let grid = Rhythm::new(8.0);
    // Georgia-like metrics at 16px on a 3-unit line.
    let font = FontRhythm::from_platform_metrics(16.0, 3, 14.67, -3.51, 11.09, 7.70);

    let before = ALLOCS.load(Ordering::Relaxed);
    let mut acc = 0.0f32;
    for i in 0..10_000u32 {
        let ascent = 14.0 + (i % 7) as f32 * 0.5;
        let line = RhythmLineMetrics::new(ascent, 3.51, 3, grid);
        let block = RhythmBlockMetrics::new(line, 3, 1);
        acc += line.paint_origin_for(grid.height(5))
            + line.baseline_above()
            + line.min_line_rhythms() as f32
            + (line.overflows_line_box() as u32) as f32
            + block.height(7)
            + block.first_height(2)
            + block.last_height(5)
            + block.rows(7) as f32
            + grid.baseline_top(&font, 3)
            + grid.baseline_between(&font, &font, 6)
            + font.line_metrics(grid).baseline_below()
            + grid.snap_up(450.0 + i as f32);
    }
    let after = ALLOCS.load(Ordering::Relaxed);
    std::hint::black_box(acc);

    assert_eq!(
        after - before,
        0,
        "line/block geometry and resolved-font spacing must not allocate"
    );
}
