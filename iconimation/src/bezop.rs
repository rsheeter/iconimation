use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};

pub(crate) trait ContainedPoint {
    /// Find a point that is contained within the subpath
    ///
    /// Meant for simplified (assume the answer is the same for the entire subpath) nonzero fill resolution.
    fn contained_point(&self) -> Option<Point>;
}

impl ContainedPoint for BezPath {
    fn contained_point(&self) -> Option<Point> {
        let Some(PathEl::MoveTo(p)) = self.elements().first() else {
            eprintln!("Subpath doesn't start with a move!");
            return None;
        };

        // our shapes are simple, just bet that a nearby point is contained
        let offsets = [0.0, 0.001, -0.001];
        offsets
            .iter()
            .flat_map(|x_off| offsets.iter().map(|y_off| Vec2::new(*x_off, *y_off)))
            .map(|offset| *p + offset)
            .find(|p| self.contains(*p))
    }
}

/// Simplified version of [Affine2D::rect_to_rect](https://github.com/googlefonts/picosvg/blob/a0bcfade7a60cbd6f47d8bfe65b6d471cee628c0/src/picosvg/svg_transform.py#L216-L263)
///
/// font_box is assumed y-up, dest_box y-down
pub fn y_up_to_y_down(font_box: Rect, dest_box: Rect) -> Affine {
    assert!(font_box.width() > 0.0);
    assert!(font_box.height() > 0.0);
    assert!(dest_box.width() > 0.0);
    assert!(dest_box.height() > 0.0);

    let (sx, sy) = (
        dest_box.width() / font_box.width(),
        dest_box.height() / font_box.height(),
    );
    let transform = Affine::IDENTITY
        // Move the font box to touch the origin
        .then_translate((-font_box.min_x(), -font_box.min_y()).into())
        // Do a flip!
        .then_scale_non_uniform(1.0, -1.0)
        // Scale to match the target box
        .then_scale_non_uniform(sx, sy);

    // Line up
    let adjusted_font_box = transform.transform_rect_bbox(font_box);
    transform.then_translate(
        (
            dest_box.min_x() - adjusted_font_box.min_x(),
            dest_box.min_y() - adjusted_font_box.min_y(),
        )
            .into(),
    )
}
