//! `rhythm_frame`: fit fluid-width media into whole rhythm rows.

use gpui::{
    div, px, relative, size, AnyElement, App, AvailableSpace, Bounds, Element, ElementId,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, RenderOnce,
    Size, Style, Styled, Window,
};

use crate::RhythmGrid;

/// A container that fits fluid-width media — images, video, embeds, any
/// content whose height follows its width instead of the rhythm — into a
/// whole number of rhythm rows, so everything after it stays on the grid at
/// any width.
///
/// The frame fills the parent's width; its height is the content's natural
/// height (`width / ratio`) snapped to whole rhythm rows, recomputed in the
/// layout pass whenever the width changes. `ratio` is width over height,
/// matching gpui `img`'s aspect-ratio convention. Content lives in a box at
/// its natural height: pinned to the frame's top edge when padding, and
/// centered behind the frame's clipping mask when cropping. Style the child
/// to fill it (`.size_full()`, plus `.object_fit(ObjectFit::Cover)` when the
/// image itself has a different ratio). The box reserves its height even
/// before an image loads — no layout shift.
///
/// By default the height rounds **up** and the leftover space — always under
/// one rhythm unit — is left below the content (pad). Call
/// [`RhythmFrame::crop`] to round **down** instead and clip the overfill.
/// With a known width the frame is unnecessary — size the block directly
/// with [`RhythmGrid::snap_up`].
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
///
/// use gpui::{img, prelude::*, px, ObjectFit};
/// use rhythm_gpui::{rhythm_frame, RhythmGrid};
///
/// fn figure(grid: RhythmGrid, crop: bool) -> impl IntoElement {
///     // Pad (default) keeps the whole image and leaves the sub-unit
///     // remainder below it; `.crop()` splits the clipped remainder between
///     // the top and bottom edges.
///     let frame = rhythm_frame(grid, 16. / 9.);
///     let frame = if crop { frame.crop() } else { frame };
///     frame.child(
///         img(PathBuf::from("photo.jpg"))
///             .size_full()
///             .object_fit(ObjectFit::Cover),
///     )
/// }
/// ```
#[derive(IntoElement)]
pub struct RhythmFrame {
    grid: RhythmGrid,
    ratio: f32,
    crop: bool,
    children: Vec<AnyElement>,
}

/// Create a [`RhythmFrame`] for content with a `width / height` ratio of
/// `ratio`; add the content with `.child()`.
///
/// # Panics
///
/// Panics when `ratio` is zero, negative, or non-finite.
pub fn rhythm_frame(grid: RhythmGrid, ratio: f32) -> RhythmFrame {
    assert!(
        ratio.is_finite() && ratio > 0.0,
        "aspect ratio must be finite and greater than zero"
    );
    RhythmFrame {
        grid,
        ratio,
        crop: false,
        children: Vec::new(),
    }
}

impl RhythmFrame {
    /// Round the height down instead of up: the natural-height content
    /// overfills the frame by less than one rhythm unit, is centered, and is
    /// clipped evenly between the top and bottom edges instead of padded.
    /// Rounding down never scales the content to the snapped height. Suits
    /// full-bleed photography; keep the default pad for diagrams and
    /// screenshots, and note that frames narrower than one row's worth of
    /// content floor to zero height.
    pub fn crop(mut self) -> Self {
        self.crop = true;
        self
    }
}

impl ParentElement for RhythmFrame {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl RenderOnce for RhythmFrame {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // The sizer is the only in-flow child, so the frame's height is the
        // snapped height. The overlaid mask contains a natural-height content
        // box (gpui's `img` cannot derive its height from a fractional width,
        // so the box sizes the content, not the reverse). Pad leaves that box
        // at the top; crop centers the inflexible box so overflow is split
        // between the top and bottom instead of delegated to `ObjectFit`.
        let mut natural = div().w_full().flex_none();
        natural.style().aspect_ratio = Some(self.ratio);
        let natural = natural.children(self.children);

        let mut mask = div().absolute().inset_0().overflow_hidden();
        if self.crop {
            mask = mask.flex().flex_row().items_center();
        }

        div()
            .relative()
            .w_full()
            .child(FrameSizer {
                grid: self.grid,
                ratio: self.ratio,
                crop: self.crop,
            })
            .child(mask.child(natural))
    }
}

/// The invisible in-flow leaf that gives the frame its height: a measured
/// Taffy node returning the snapped height for the width it is offered.
struct FrameSizer {
    grid: RhythmGrid,
    ratio: f32,
    crop: bool,
}

/// The frame height at `width`: the natural `width / ratio` snapped to whole
/// rhythm rows — up for pad, down for crop.
fn snapped_height(grid: RhythmGrid, ratio: f32, crop: bool, width: Pixels) -> Pixels {
    let natural = px(f32::from(width) / ratio);
    if crop {
        grid.snap_down(natural)
    } else {
        grid.snap_up(natural)
    }
}

impl IntoElement for FrameSizer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for FrameSizer {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let grid = self.grid;
        let ratio = self.ratio;
        let crop = self.crop;
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        // Taffy may probe with min-/max-content available space; without a
        // definite width the height is unknowable, so report zero and let
        // the definite pass size the frame.
        let layout_id = window.request_measured_layout(style, move |known, available, _, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(width) => Some(width),
                AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
            });
            match width {
                Some(width) => size(width, snapped_height(grid, ratio, crop, width)),
                None => Size::default(),
            }
        });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapped_height_pads_up_and_crops_down() {
        let grid = RhythmGrid::new(px(8.0));
        // 800px wide at 16:9 → 450px natural height.
        assert_eq!(snapped_height(grid, 16. / 9., false, px(800.0)), px(456.0));
        assert_eq!(snapped_height(grid, 16. / 9., true, px(800.0)), px(448.0));
        // Exact multiples pass through untouched in both modes.
        assert_eq!(snapped_height(grid, 2.0, false, px(96.0)), px(48.0));
        assert_eq!(snapped_height(grid, 2.0, true, px(96.0)), px(48.0));
    }

    #[test]
    fn frame_collects_children_and_the_crop_flag() {
        let grid = RhythmGrid::new(px(8.0));
        let mut frame = rhythm_frame(grid, 16. / 9.);
        assert!(!frame.crop);
        frame.extend([gpui::Empty.into_any_element()]);
        let frame = frame.crop();
        assert!(frame.crop);
        assert_eq!(frame.children.len(), 1);
    }

    #[test]
    #[should_panic(expected = "aspect ratio must be finite and greater than zero")]
    fn frame_rejects_a_non_positive_ratio() {
        let _ = rhythm_frame(RhythmGrid::new(px(8.0)), 0.0);
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn frame_layout_pads_at_the_bottom_and_crops_both_edges(cx: &mut gpui::TestAppContext) {
        use gpui::{point, AvailableSpace, InteractiveElement};

        let cx = cx.add_empty_window();
        let grid = RhythmGrid::new(px(8.0));

        cx.draw(
            point(px(0.0), px(0.0)),
            size(
                AvailableSpace::Definite(px(800.0)),
                AvailableSpace::MaxContent,
            ),
            |_, _| {
                div()
                    .w(px(800.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .debug_selector(|| "pad-frame".into())
                            .child(
                                rhythm_frame(grid, 16. / 9.).child(
                                    div().size_full().debug_selector(|| "pad-content".into()),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .debug_selector(|| "crop-frame".into())
                            .child(
                                rhythm_frame(grid, 16. / 9.).crop().child(
                                    div().size_full().debug_selector(|| "crop-content".into()),
                                ),
                            ),
                    )
            },
        );

        let pad_frame = cx.debug_bounds("pad-frame").expect("pad frame bounds");
        let pad_content = cx.debug_bounds("pad-content").expect("pad content bounds");
        assert_eq!(pad_frame.size.height, px(456.0));
        assert_eq!(pad_content.size.height, px(450.0));
        assert_eq!(pad_content.origin.y, pad_frame.origin.y);

        let crop_frame = cx.debug_bounds("crop-frame").expect("crop frame bounds");
        let crop_content = cx
            .debug_bounds("crop-content")
            .expect("crop content bounds");
        assert_eq!(crop_frame.size.height, px(448.0));
        assert_eq!(crop_content.size.height, px(450.0));

        let top_overflow = crop_frame.origin.y - crop_content.origin.y;
        let bottom_overflow = (crop_content.origin.y + crop_content.size.height)
            - (crop_frame.origin.y + crop_frame.size.height);
        assert_eq!(top_overflow, px(1.0));
        assert_eq!(bottom_overflow, px(1.0));
    }
}
