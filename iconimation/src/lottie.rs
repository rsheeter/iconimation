//! Converts [`AnimatedGlyph`] to [`Lottie`]`

use std::time::Duration;

use bodymovin::{
    layers::{AnyLayer, Shape as ShapeLayer, ShapeMixin},
    properties::{
        Bezier2d, BezierEase, ControlPoint2d, MultiDimensionalKeyframe, Property, ShapeKeyframe,
        ShapeValue, Value,
    },
    shapes::{AnyShape, Group as LottieGroup, SubPath, Transform as ShapeTransform},
    Bodymovin as Lottie,
};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use write_fonts::pens::{write_to_pen, TransformPen};

use crate::{
    animated_glyph::{AnimatedGlyph, Element, Group},
    animator::{Animated, IntervalPosition, MotionBender, ToDeliveryFormat},
    error::{Error, ToDeliveryError},
    path_commands,
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
    lottie_center: [f64; 2],
    bender: &'a dyn MotionBender,
    duration: Duration,
}

impl<'a> LottieWriter<'a> {
    fn new(font_box: Rect, bender: &'a dyn MotionBender, duration: Duration) -> Self {
        // Simplified version of [Affine2D::rect_to_rect](https://github.com/googlefonts/picosvg/blob/a0bcfade7a60cbd6f47d8bfe65b6d471cee628c0/src/picosvg/svg_transform.py#L216-L263)
        let font_to_lottie = Affine::IDENTITY
            // Move the font box to touch the origin
            .then_translate((-font_box.min_x(), -font_box.min_y()).into())
            // Do a flip to correct for font being Y-up, Lottie Y-down
            .then_scale_non_uniform(1.0, -1.0)
            // Move into the viewbox
            .then_translate((0.0, font_box.height()).into());

        Self {
            frame_rate: 60.0,
            font_to_lottie,
            lottie_center: [font_box.width() / 2.0, font_box.height() / 2.0],
            bender,
            duration,
        }
    }

    fn create_group(&self, group: &Group) -> Result<LottieGroup, crate::error::ToDeliveryError> {
        let mut items: Vec<_> = group
            .children
            .iter()
            .flat_map(|c| {
                let items = match c {
                    Element::Group(g) => self.create_group(g).map(|g| vec![AnyShape::Group(g)]),
                    Element::Path(p) => self
                        .create_subpaths(p)
                        .map(|p| p.into_iter().map(|p| AnyShape::Shape(p)).collect()),
                };
                match items {
                    Ok(vec) => vec.into_iter().map(|s| Ok(s)).collect(),
                    Err(e) => vec![Err(e)],
                }
            })
            .collect::<Result<_, _>>()?;

        // Animate relative to the center of the shape(s)
        // https://lottiefiles.github.io/lottie-docs/concepts/#transform
        // notes that anchor and position need to match for this
        let center = Property {
            value: Value::Fixed(self.lottie_center.to_vec()),
            ..Default::default()
        };
        let mut transform = ShapeTransform {
            anchor_point: center.clone(),
            position: center.clone(),
            ..Default::default()
        };
        if let Some(rotate) = group.rotate.as_ref() {
            if rotate.is_animated() {
                transform.rotation.animated = 1;
                transform.rotation.value = Value::Animated(
                    rotate
                        .iter()
                        .map(|(pos, value)| self.create_simple_keyframe(*pos, vec![*value]))
                        .collect(),
                );
            } else {
                transform.rotation.value = Value::Fixed(*rotate.first());
            }
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

    // https://lottiefiles.github.io/lottie-docs/playground/json_editor/ doesn't play if there is no ease
    // TODO: use the motion curve
    fn default_ease(&self) -> BezierEase {
        BezierEase::_2D(Bezier2d {
            in_value: ControlPoint2d { x: 0.6, y: 1.0 },
            out_value: ControlPoint2d { x: 0.4, y: 0.0 },
        })
    }

    fn create_simple_keyframe(
        &self,
        pos: IntervalPosition,
        value: Vec<f64>,
    ) -> MultiDimensionalKeyframe {
        MultiDimensionalKeyframe {
            start_time: self.frame_rate * self.duration.as_secs_f64() * pos.into_inner(),
            start_value: Some(value),
            bezier: Some(self.default_ease()),
            ..Default::default()
        }
    }

    fn create_path_frame(&self, pos: IntervalPosition, value: ShapeValue) -> ShapeKeyframe {
        ShapeKeyframe {
            start_time: self.frame_rate * self.duration.as_secs_f64() * pos.into_inner(),
            start_value: Some(vec![value]),
            bezier: Some(self.default_ease()),
            ..Default::default()
        }
    }

    fn create_subpaths(
        &self,
        path: &Animated<BezPath>,
    ) -> Result<Vec<SubPath>, crate::error::ToDeliveryError> {
        // Interpolation compatible paths?
        // TODO: figure out when to swap when *not* compatible, e.g. for singlesub at FILL>=0.99
        if path.len() > 1 {
            let cmd_seq = path_commands(path.first());
            for (pos, path) in path.iter().skip(1) {
                let cmds = path_commands(path);
                if cmd_seq != cmds {
                    return Err(ToDeliveryError::IncompatiblePath(*pos));
                }
            }
        }

        let mut subpaths: Vec<_> = path
            .iter()
            .map(|(t, p)| {
                subpaths_for_glyph(&p, self.font_to_lottie).map(|subpaths| (*t, subpaths))
            })
            .collect::<Result<_, _>>()
            .map_err(ToDeliveryError::PathConversionError)?;

        let (start_pos, mut start_subpaths) = subpaths.pop().unwrap(); // animated must have an entry

        // Create keyframes for animated subpaths
        // At each timeslot we can have many subpaths. They don't necessarily all animate.
        let animated_indices: Vec<_> = (0..start_subpaths.len())
            .filter(|i| {
                let start_path = &start_subpaths[*i];
                subpaths.iter().any(|(_, paths)| *start_path != paths[*i])
            })
            .collect();

        for i in animated_indices {
            let start_path = start_subpaths.get_mut(i).unwrap();
            let Value::Fixed(start_value) = &start_path.vertices.value else {
                return Err(ToDeliveryError::UnexpectedAnimation(start_pos));
            };

            let mut keyframes = Vec::with_capacity(subpaths.len() + 1);
            keyframes.push(self.create_path_frame(start_pos, start_value.clone()));
            for (pos, value) in subpaths
                .iter()
                .map(|(pos, paths)| (*pos, paths.get(i).unwrap()))
            {
                let Value::Fixed(value) = &value.vertices.value else {
                    return Err(ToDeliveryError::UnexpectedAnimation(pos));
                };
                keyframes.push(self.create_path_frame(pos, value.clone()));
            }

            start_path.vertices.animated = 1;
            start_path.vertices.value = Value::Animated(keyframes);
        }

        Ok(start_subpaths)
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
