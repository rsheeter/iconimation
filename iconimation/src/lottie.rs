//! Converts [`AnimatedGlyph`] to [`Lottie`]`

use std::time::Duration;

use bodymovin::{
    helpers::Transform,
    layers::{AnyLayer, Shape as ShapeLayer, ShapeMixin},
    properties::{Property, ShapeValue, SplittableMultiDimensional, Value},
    shapes::{AnyShape, Group as LottieGroup, SubPath, Transform as ShapeTransform},
    Bodymovin as Lottie,
};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use write_fonts::pens::{write_to_pen, TransformPen};

use crate::{
    animated_glyph::{AnimatedGlyph, Element, Group},
    animator::{Animated, MotionBender, ToDeliveryFormat},
    error::{AnimationError, Error, ToDeliveryError},
    shape_pen::{bez_to_shape, SubPathPen},
};

/// Returns a [SubPath] in Lottie units for each subpath of a glyph
fn subpaths_for_glyph(
    path: &BezPath,
    font_units_to_lottie_units: Affine,
) -> Result<Vec<SubPath>, Error> {
    let mut subpath_pen = SubPathPen::default();
    let mut transform_pen = TransformPen::new(&mut subpath_pen, font_units_to_lottie_units);

    write_to_pen(path, &mut transform_pen);

    Ok(subpath_pen.paths().iter().map(bez_to_shape).collect())
}

struct LottieWriter<'a> {
    frame_rate: f64,
    font_to_lottie: Affine,
    bender: &'a dyn MotionBender,
    duration: Duration,
}

impl<'a> LottieWriter<'a> {
    fn new(font_box: Rect, bender: &'a dyn MotionBender, duration: Duration) -> Self {
        // Simplified version of [Affine2D::rect_to_rect](https://github.com/googlefonts/picosvg/blob/a0bcfade7a60cbd6f47d8bfe65b6d471cee628c0/src/picosvg/svg_transform.py#L216-L263)
        let font_to_lottie = Affine::IDENTITY
            // Move the font box to touch the origin
            .then_translate((-font_box.min_x(), -font_box.min_y()).into())
            // Do a flip!
            .then_scale_non_uniform(1.0, -1.0);
        Self {
            frame_rate: 60.0,
            font_to_lottie,
            bender,
            duration,
        }
    }

    fn create_group(&self, group: &Group) -> Result<LottieGroup, crate::error::ToDeliveryError> {
        let mut items: Vec<_> = group
            .children
            .iter()
            .map(|c| match c {
                Element::Group(g) => self.create_group(g).map(|g| AnyShape::Group(g)),
                Element::Path(p) => self.create_path(p).map(|p| AnyShape::Shape(p)),
            })
            .collect::<Result<_, _>>()?;

        let transform = ShapeTransform::default();
        if group.rotate.is_some() {
            todo!("rotate")
        }
        if group.uniform_scale.is_some() {
            todo!("uniform scale")
        }

        // de facto standard is shape(s), fill, transform
        items.push(AnyShape::Fill(Default::default()));
        items.push(AnyShape::Transform(transform));
        Ok(LottieGroup {
            items,
            ..Default::default()
        })
    }

    fn create_path(
        &self,
        path: &Animated<BezPath>,
    ) -> Result<SubPath, crate::error::ToDeliveryError> {
        let subpaths: Vec<_> = path
            .iter()
            .map(|(t, p)| {
                subpaths_for_glyph(&p, self.font_to_lottie).map(|subpaths| (*t, subpaths))
            })
            .collect::<Result<_, _>>()
            .map_err(ToDeliveryError::PathConversionError)?;
        todo!()
    }
}

impl ToDeliveryFormat for Lottie {
    fn generate(
        glyph: &AnimatedGlyph,
        bender: &dyn MotionBender,
        duration: Duration,
    ) -> Result<Self, crate::error::ToDeliveryError> {
        let writer = LottieWriter::new(glyph.font_drawbox, bender, duration);
        let group = writer.create_group(&glyph.contents)?;
        let lottie = Lottie {
            in_point: 0.0,
            out_point: duration.as_secs_f64() * writer.frame_rate,
            frame_rate: writer.frame_rate,
            width: glyph.font_drawbox.width() as i64,
            height: glyph.font_drawbox.height() as i64,
            layers: vec![AnyLayer::Shape(ShapeLayer {
                in_point: 0.0,
                out_point: duration.as_secs_f64() * writer.frame_rate,
                transform: Transform {
                    position: SplittableMultiDimensional::Uniform(Property {
                        value: Value::Fixed(vec![
                            glyph.font_drawbox.width() / 2.0,
                            glyph.font_drawbox.height() / 2.0,
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                mixin: ShapeMixin {
                    shapes: vec![AnyShape::Group(group)],
                    ..Default::default()
                },
                ..Default::default()
            })],
            ..Default::default()
        };
        Ok(lottie)
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
