//! An [OutlinePen] that generates [SubPath]'s for subpaths.
//!
//! Based on <https://github.com/rsheeter/read-fonts-wasm>

use bodymovin::{
    properties::{Property, ShapeValue, Value},
    shapes::SubPath,
};
use kurbo::{BezPath, Point, Shape as KShape};
use skrifa::outline::OutlinePen;

#[derive(Default)]
pub struct SubPathPen {
    paths: Vec<BezPath>,
}

impl SubPathPen {
    fn active_subpath(&mut self) -> &mut BezPath {
        if self.paths.is_empty() {
            self.paths.push(BezPath::new());
        }
        return self.paths.last_mut().unwrap();
    }

    pub fn paths(self) -> Vec<BezPath> {
        self.paths
    }

    pub fn into_shapes(self) -> Vec<(BezPath, SubPath)> {
        self.paths
            .into_iter()
            .map(|b| {
                let shape = bez_to_shape(&b);
                (b, shape)
            })
            .collect()
    }
}

impl OutlinePen for SubPathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.paths.push(BezPath::new());
        self.active_subpath().move_to((x as f64, y as f64));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.active_subpath().line_to((x as f64, y as f64));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.active_subpath()
            .quad_to((cx0 as f64, cy0 as f64), (x as f64, y as f64));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.active_subpath().curve_to(
            (cx0 as f64, cy0 as f64),
            (cx1 as f64, cy1 as f64),
            (x as f64, y as f64),
        );
    }

    fn close(&mut self) {
        self.active_subpath().close_path();
    }
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

pub fn bez_to_shape(path: &BezPath) -> SubPath {
    // eprintln!("bez to shape, cbox {:?}", path.control_box());

    let mut value = ShapeValue::default();
    for el in path.iter() {
        let last_on: Point = value.vertices.last().cloned().unwrap_or_default().into();
        match el {
            kurbo::PathEl::MoveTo(p) => {
                value.vertices.push(p.into());
                value.out_point.push(Point::ZERO.into());
                value.in_point.push(Point::ZERO.into());
            }
            kurbo::PathEl::LineTo(p) => add_cubic(&mut value, last_on, p, p),
            kurbo::PathEl::QuadTo(control, end) => {
                // https://pomax.github.io/bezierinfo/#reordering
                let c0 = last_on.one_third() + control.two_thirds().to_vec2();
                let c1 = control.two_thirds() + end.one_third().to_vec2();
                add_cubic(&mut value, c0, c1, end);
            }
            kurbo::PathEl::CurveTo(c0, c1, end) => add_cubic(&mut value, c0, c1, end),
            kurbo::PathEl::ClosePath => value.closed = Some(true),
        }
    }
    if value.closed.is_none() {
        value.closed = Some(
            value.vertices.first().cloned().unwrap_or_default()
                == value.vertices.last().cloned().unwrap_or_default(),
        );
    }
    SubPath {
        vertices: Property {
            value: Value::Fixed(value),
            ..Default::default()
        },
        // 1.0 = Clockwise = positive area
        // 3.0 = Counter-Clockwise = negative area
        direction: if path.area() > 0.0 {
            Some(1.0)
        } else {
            Some(3.0)
        },
        ..Default::default()
    }
}
