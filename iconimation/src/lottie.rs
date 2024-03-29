//! Create's Lottie's from Animation's

use bodymovin::{
    layers::{AnyLayer, ShapeMixin},
    properties::{
        Bezier2d, BezierEase, ControlPoint2d, MultiDimensionalKeyframe, Property, ShapeKeyframe,
        ShapeValue, Value,
    },
    shapes::{AnyShape, Fill, Group, SubPath, Transform},
    Bodymovin as Lottie,
};
use kurbo::{BezPath, PathEl, Point, Shape};

use crate::{
    error::LottieError,
    ir::{self, Element, FromAnimation, Keyframed},
    path_commands,
};

impl FromAnimation for Lottie {
    type Err = LottieError;

    fn from_animation(animation: &crate::ir::Animation) -> Result<Self, Self::Err> {
        let root_group = to_lottie_group(&animation.root)?;
        Ok(Lottie {
            in_point: 0.0,
            out_point: animation.frames,
            frame_rate: animation.frame_rate,
            width: animation.width as i64,
            height: animation.height as i64,
            layers: vec![AnyLayer::Shape(bodymovin::layers::Shape {
                in_point: 0.0,
                out_point: 60.0, // 60fps total animation = 1s
                mixin: ShapeMixin {
                    shapes: vec![AnyShape::Group(root_group)],
                    ..Default::default()
                },
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

fn to_lottie_group(group: &ir::Group) -> Result<Group, LottieError> {
    // de facto standard for Lottie is groups contains shape(s), fill, transform
    let mut items: Vec<_> = group
        .children
        .iter()
        .map(|e| match e {
            Element::Group(g) => to_lottie_group(g).map(|g| vec![AnyShape::Group(g)]),
            Element::Shape(s) => {
                to_lottie_subpath(s).map(|s| s.into_iter().map(AnyShape::Shape).collect())
            }
        })
        .collect::<Result<Vec<_>, LottieError>>()?
        .into_iter()
        .flatten()
        .collect();

    let mut fill = Fill::default();
    if let Some((r, g, b)) = group.fill {
        fill.color = Property {
            value: Value::Fixed(vec![r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]),
            ..Default::default()
        };
    }
    items.push(AnyShape::Fill(fill));
    items.push(AnyShape::Transform(to_lottie_transform(group)));

    Ok(Group {
        items,
        ..Default::default()
    })
}

fn to_lottie_transform(group: &ir::Group) -> Transform {
    let mut transform = Transform::default();
    let (center_x, center_y) = (group.center.x, group.center.y);
    eprintln!("to_lottie_transform center {:?}", group.center);
    transform.anchor_point.value = Value::Fixed(vec![center_x, center_y]);

    transform.rotation.animated = group.rotate.is_animated() as i8;
    transform.rotation.value = if group.rotate.is_animated() {
        Value::Animated(
            group
                .rotate
                .iter()
                .map(|keyframe| MultiDimensionalKeyframe {
                    start_time: keyframe.frame,
                    start_value: Some(vec![keyframe.value]),
                    bezier: Some(default_ease()),
                    ..Default::default()
                })
                .collect(),
        )
    } else {
        Value::Fixed(group.rotate.earliest().value)
    };

    transform.scale.animated = group.scale.is_animated() as i8;
    transform.scale.value = if group.scale.is_animated() {
        Value::Animated(
            group
                .scale
                .iter()
                .map(|keyframe| MultiDimensionalKeyframe {
                    start_time: keyframe.frame,
                    start_value: Some(vec![keyframe.value.0, keyframe.value.1]),
                    bezier: Some(default_ease()),
                    ..Default::default()
                })
                .collect(),
        )
    } else {
        let value = group.scale.earliest().value;
        Value::Fixed(vec![value.0, value.1])
    };

    transform.position.animated = group.translate.is_animated() as i8;
    transform.position.value = if group.translate.is_animated() {
        Value::Animated(
            group
                .translate
                .iter()
                .map(|keyframe| MultiDimensionalKeyframe {
                    start_time: keyframe.frame,
                    start_value: Some(vec![
                        center_x + keyframe.value.x,
                        center_y + keyframe.value.y,
                    ]),
                    bezier: Some(default_ease()),
                    ..Default::default()
                })
                .collect(),
        )
    } else {
        let value = group.translate.earliest().value;
        Value::Fixed(vec![center_x + value.x, center_y + value.y])
    };

    transform
}

fn default_ease() -> BezierEase {
    // If https://lottiefiles.github.io/lottie-docs/playground/json_editor/ is to be believed
    // the bezier ease is usually required since we rarely want to "hold"
    BezierEase::_2D(Bezier2d {
        // the control point incoming to destination
        in_value: ControlPoint2d { x: 0.6, y: 1.0 },
        // the control point outgoing from origin
        out_value: ControlPoint2d { x: 0.4, y: 0.0 },
    })
}

fn to_lottie_subpath(path: &Keyframed<BezPath>) -> Result<Vec<SubPath>, LottieError> {
    // https://lottiefiles.github.io/lottie-docs/playground/json_editor/ doesn't play if there is no ease
    let ease = default_ease();

    // In a mildly confusing turn of events an *animated* subpath has keyframes with
    // vectors of paths while a static one just gets a single continuous path so what we
    // produce varies based on whether we're animated
    let first_frame = path.earliest();
    if path.len() < 2 {
        return Ok(first_frame.subpaths().iter().map(create_subpath).collect());
    }

    // We're animated!

    // TODO: support incompatible paths by cutting between them
    // For now just reject incompatible paths
    let first_frame_cmds = path_commands(&first_frame.value);
    if !path
        .iter()
        .map(|p| path_commands(&p.value))
        .all(|commands| first_frame_cmds == commands)
    {
        return Err(LottieError::IncompatiblePaths(path.clone()));
    }

    // The shape is animated, make a single subpath whose keyframes have lots of static paths
    let mut subpath = SubPath::default();
    subpath.vertices.animated = 1;
    let mut keyframes = Vec::with_capacity(path.len());

    if path.len() > 2 {
        panic!("TODO: support > 2 path keyframes");
    }

    for ir_keyframe in path.iter() {
        keyframes.push(ShapeKeyframe {
            start_time: ir_keyframe.frame,
            start_value: Some(
                ir_keyframe
                    .subpaths()
                    .iter()
                    .map(create_shapevalue)
                    .collect(),
            ),
            // no ease, no render
            bezier: Some(ease.clone()),
            ..Default::default()
        })
    }

    subpath.vertices.value = Value::Animated(keyframes);
    Ok(vec![subpath])
}

trait Thirds {
    fn one_third(&self) -> Self;
    fn two_thirds(&self) -> Self;
}

impl Thirds for Point {
    fn one_third(&self) -> Self {
        (self.x / 3.0, self.y / 3.0).into()
    }

    fn two_thirds(&self) -> Self {
        (self.x * 2.0 / 3.0, self.y * 2.0 / 3.0).into()
    }
}

/// Add a cubic with absolute coordinates to a Lottie b-spline
fn add_cubic(shape: &mut ShapeValue, c0: Point, c1: Point, end: Point) {
    // Shape is a cubic B-Spline
    //  vertices are oncurve points, absolute coordinates
    //  in_point[i] is the "incoming" control point for vertices[i+1], relative coordinate.
    //  out_point[i] is the "outgoing" control point for vertices[i], relative coordinate.
    // Contrast with a typical cubic (https://developer.mozilla.org/en-US/docs/Web/SVG/Tutorial/Paths#b%C3%A9zier_curves)
    // Cubic[i] in absolute terms is formed by:
    //      Start:          vertices[i]
    //      First control:  vertices[i] + outgoing[i]
    //      Second control: vertices[i + 1] + incoming[i]
    //      End:            vertices[i + 1]
    // If closed 1 past the end of vertices is vertices[0]

    let start: Point = shape
        .vertices
        .last()
        .map(|coords| (*coords).into())
        .unwrap_or_default();
    let i = shape.vertices.len() - 1;

    shape.out_point.push(Point::ZERO.into());
    shape.in_point.push(Point::ZERO.into());

    shape.out_point[i] = (c0 - start).into();
    shape.in_point[i + 1] = (c1 - end).into();
    shape.vertices.push(end.into());
}

fn create_subpath(subpath: &BezPath) -> SubPath {
    // eprintln!("create_subpath, cbox {:?}", path.control_box());
    SubPath {
        vertices: Property {
            value: Value::Fixed(create_shapevalue(subpath)),
            ..Default::default()
        },
        // 1.0 = Clockwise = positive area
        // 3.0 = Counter-Clockwise = negative area
        direction: if subpath.area() > 0.0 {
            Some(1.0)
        } else {
            Some(3.0)
        },
        ..Default::default()
    }
}

fn create_shapevalue(subpath: &BezPath) -> ShapeValue {
    let mut value = ShapeValue::default();
    for el in subpath.iter() {
        let last_on: Point = value.vertices.last().cloned().unwrap_or_default().into();
        match el {
            PathEl::MoveTo(p) => {
                assert!(value.vertices.is_empty(), "Multiple moves is not a subpath");
                value.vertices.push((p).into());
                value.out_point.push(Point::ZERO.into());
                value.in_point.push(Point::ZERO.into());
            }
            PathEl::LineTo(p) => add_cubic(&mut value, last_on, p, p),
            PathEl::QuadTo(control, end) => {
                // https://pomax.github.io/bezierinfo/#reordering
                let c0 = last_on.one_third() + control.two_thirds().to_vec2();
                let c1 = control.two_thirds() + end.one_third().to_vec2();
                add_cubic(&mut value, c0, c1, end);
            }
            PathEl::CurveTo(c0, c1, end) => add_cubic(&mut value, c0, c1, end),
            PathEl::ClosePath => value.closed = Some(true),
        }
    }
    if value.closed.is_none() {
        value.closed = Some(
            value.vertices.first().cloned().unwrap_or_default()
                == value.vertices.last().cloned().unwrap_or_default(),
        );
    }
    value
}

#[cfg(test)]
mod tests {}
