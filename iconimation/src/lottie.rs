//! Converts [`AnimatedGlyph`] to [`Lottie`]`

use bodymovin::{
    properties::{ShapeValue, Value},
    shapes::{AnyShape, SubPath},
    Bodymovin as Lottie,
};
use kurbo::{BezPath, Point, Rect, Shape};

use crate::{animated_glyph::AnimatedGlyph, error::Error};

impl From<AnimatedGlyph> for Lottie {
    fn from(value: AnimatedGlyph) -> Self {
        todo!()
    }
}

pub(crate) trait LottieGeometry {
    fn bounding_box(&self) -> Option<Rect>;

    fn to_bez(&self) -> Result<BezPath, Error>;
}

impl LottieGeometry for AnyShape {
    fn bounding_box(&self) -> Option<Rect> {
        self.to_bez().map(|b| b.bounding_box()).ok()
    }

    fn to_bez(&self) -> Result<BezPath, Error> {
        Ok(match self {
            AnyShape::Shape(subpath) => bez_for_subpath(subpath),
            _ => todo!("to_bez {self:?}"),
        })
    }
}

fn add_shape_to_path(path: &mut BezPath, shape: &ShapeValue) {
    if !shape.vertices.is_empty() {
        path.move_to(shape.vertices[0]);
    }
    // See SubPathPen for explanation of coords
    for i in 0..shape.vertices.len() {
        let start: Point = shape.vertices[i].into();

        let end: Point = if i + 1 < shape.vertices.len() {
            shape.vertices[i + 1].into()
        } else if shape.closed.unwrap_or_default() {
            shape.vertices[0].into()
        } else {
            break;
        };
        let c0 = start + shape.out_point[i];
        let c1 = end + shape.in_point[i];

        path.curve_to(c0, c1, end);
    }

    if shape.closed.unwrap_or_default() {
        path.close_path();
    }
}

fn bez_for_subpath(subpath: &SubPath) -> BezPath {
    let mut path = BezPath::new();
    match &subpath.vertices.value {
        Value::Fixed(shape) => add_shape_to_path(&mut path, shape),
        Value::Animated(value) => {
            let first_keyframe = value
                .iter()
                .reduce(|acc, e| {
                    if acc.start_time <= e.start_time {
                        acc
                    } else {
                        e
                    }
                })
                .and_then(|sk| sk.start_value.as_ref());
            if let Some(first_keyframe) = first_keyframe {
                for shape in first_keyframe.iter() {
                    add_shape_to_path(&mut path, shape);
                }
            }
        }
    };
    path
}
