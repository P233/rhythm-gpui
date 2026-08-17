//! Structural performance contract: hot-path geometry allocates nothing.
//!
//! A counting global allocator replaces ns-level benchmark thresholds — pure
//! f32 math varies below measurement noise, but allocation freedom is checkable
//! exactly. Locking and text-system access are structural source contracts
//! outside this allocator's scope. The single `#[test]` keeps the harness quiet
//! while the counter window is open.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "gpui")]
use gpui::px;
#[cfg(feature = "gpui")]
use rhythm_gpui::RhythmGrid;
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
    #[cfg(feature = "gpui")]
    let typed_grid = RhythmGrid::new(px(8.0));
    // Georgia-like metrics at 16px on a 3-unit line.
    let font = FontRhythm::from_platform_metrics(16.0, 3, 14.67, -3.51, 11.09, 7.70);

    let before = ALLOCS.load(Ordering::Relaxed);
    let mut acc = 0.0f32;
    for i in 0..10_000u32 {
        // Alternate across the 24px floor so the overflow probes exercise
        // both the fitting and overflowing line-box paths.
        let ascent = 14.0 + (i % 16) as f32;
        let line = RhythmLineMetrics::new(ascent, 3.51, 3, grid);
        let grown = RhythmLineMetrics::at_least(ascent, 3.51, 3, grid);
        let block = RhythmBlockMetrics::new(line, 3, 1);
        let cap_block = RhythmBlockMetrics::cap(line, 11.09, 3, 0);
        acc += grid.spacing(5)
            + line.paint_origin_for(grid.height(5))
            + line.baseline_above()
            + grown.line_height()
            + line.min_line_rhythms() as f32
            + (line.overflows_line_box() as u32) as f32
            + block.opening()
            + block.closing()
            + block.first_baseline()
            + block.height(7)
            + block.first_height(2)
            + block.middle_height(4)
            + block.last_height(5)
            + block.rows(7) as f32
            + block.first_rows(2) as f32
            + block.middle_rows(4) as f32
            + block.last_rows(5) as f32
            + block.baseline_at_row(i64::from(block.first_rows(2)))
            + cap_block.baseline_at_row(i64::from(cap_block.first_rows(2)))
            + grid.baseline_top(&font, 3)
            + grid.baseline_between(&font, &font, 6)
            + font.line_metrics(grid).baseline_below()
            + RhythmLineMetrics::covering(&[line, font.line_metrics(grid)], grid).line_height()
            + grid.snap_up(450.0 + i as f32);

        #[cfg(feature = "gpui")]
        {
            let typed_line = typed_grid.line_metrics(px(ascent), px(3.51), 3);
            let typed_grown = typed_grid.line_metrics_at_least(px(ascent), px(3.51), 3);
            acc += f32::from(typed_grid.spacing(5))
                + f32::from(typed_grown.line_height_px())
                + f32::from(typed_line.ascent_px())
                + f32::from(typed_line.descent_px())
                + f32::from(typed_line.line_height_px())
                + f32::from(typed_line.half_leading_px())
                + f32::from(typed_line.baseline_above_px())
                + f32::from(typed_line.baseline_below_px())
                + f32::from(typed_line.paint_origin_for_px(px(grid.height(5))))
                + f32::from(block.opening_px())
                + f32::from(block.closing_px())
                + f32::from(block.first_baseline_px())
                + f32::from(block.height_px(7))
                + f32::from(block.first_height_px(2))
                + f32::from(block.middle_height_px(4))
                + f32::from(block.last_height_px(5))
                + f32::from(block.baseline_at_row_px(i64::from(block.first_rows(2))));
        }
    }
    let after = ALLOCS.load(Ordering::Relaxed);
    std::hint::black_box(acc);

    assert_eq!(
        after - before,
        0,
        "line/block geometry and resolved-font spacing must not allocate"
    );
}
