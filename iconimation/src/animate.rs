//! Simple algorithmic animations
//!
//! Typically supports both a whole-icon and parts mode where parts animate offset slightly in time.

use std::fmt::Debug;

use bodymovin::properties::{Bezier2d, BezierEase, MultiDimensionalKeyframe, Property};
use bodymovin::properties::{ControlPoint2d, Value};
use bodymovin::shapes::{AnyShape, Fill, Group, Transform};
use kurbo::{BezPath, PathEl, Point, Rect, Shape, Vec2};
use ordered_float::OrderedFloat;

use crate::{bez_for_subpath, Error, ToLottie};

#[derive(Debug)]
pub enum Animation<'a> {
    None(&'a dyn ToLottie),
    PulseWhole(&'a dyn ToLottie),
    PulseParts(&'a dyn ToLottie),
    TwirlWhole(&'a dyn ToLottie),
    TwirlParts(&'a dyn ToLottie),
}

impl<'a> ToLottie for Animation<'a> {
    fn create(&self, start: f64, end: f64, dest_box: Rect) -> Result<Vec<AnyShape>, Error> {
        match self {
            Animation::None(to_lottie) => to_lottie.create(start, end, dest_box),
            Animation::PulseWhole(to_lottie) => {
                let shapes = to_lottie.create(start, end, dest_box)?;
                Ok(vec![pulse(start, end, 0, shapes)])
            }
            Animation::TwirlWhole(to_lottie) => {
                let shapes = to_lottie.create(start, end, dest_box)?;
                Ok(vec![twirl(start, end, 0, shapes)])
            }
            Animation::PulseParts(to_lottie) => {
                let shapes = to_lottie.create(start, end, dest_box)?;
                Ok(group_icon_parts(shapes)
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| pulse(start, end, i, s))
                    .collect())
            }
            Animation::TwirlParts(to_lottie) => {
                let shapes = to_lottie.create(start, end, dest_box)?;
                Ok(group_icon_parts(shapes)
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| twirl(start, end, i, s))
                    .collect())
            }
        }
    }
}

fn default_ease() -> BezierEase {
    // If https://lottiefiles.github.io/lottie-docs/playground/json_editor/ is to be believed
    // the bezier ease is usually required since we rarely want to hold
    BezierEase::_2D(Bezier2d {
        // the control point incoming to destination
        in_value: ControlPoint2d { x: 0.6, y: 1.0 },
        // the control point outgoing from origin
        out_value: ControlPoint2d { x: 0.4, y: 0.0 },
    })
}

/// Find a point that is contained within the subpath
///
/// Meant for simplified (assume the answer is the same for the entire subpath) nonzero fill resolution.
pub fn a_contained_point(subpath: &BezPath) -> Option<Point> {
    let Some(PathEl::MoveTo(p)) = subpath.elements().first() else {
        eprintln!("Subpath doesn't start with a move!");
        return None;
    };

    // our shapes are simple, just bet that a nearby point is contained
    let offsets = [0.0, 0.001, -0.001];
    offsets
        .iter()
        .flat_map(|x_off| offsets.iter().map(|y_off| Vec2::new(*x_off, *y_off)))
        .map(|offset| *p + offset)
        .find(|p| subpath.contains(*p))
}

/// Piece-wise animation wants to animate "parts" as the eye perceives them; try to so group
///
/// Most importantly, if we have a shape and hole(s) cut out of it they should be together.
///
/// Make some simplifying assumptions:
///
/// 1. Icons don't typically use one subpath to cut a hole in many other subpaths
/// 1. Icons typically fully contain the holepunch within the ... punchee?
///
/// Since we are using non-zero fill, figure out shape by shape what the winding value is. Initially I thought
/// we could simply look at the direction from [`BezPath::area`] but that ofc isn't enough to know if the final
/// winding is nonzero.
pub fn group_icon_parts(shapes: Vec<AnyShape>) -> Vec<Vec<AnyShape>> {
    // TODO: generalize. For now just assume input is all paths.
    let paths: Vec<_> = shapes.iter().map(|s| s.to_bez().unwrap()).collect();

    // Figure out what is/isn't filled
    let filled: Vec<_> = paths
        .iter()
        .map(|bez| {
            let Some(contained) = a_contained_point(bez) else {
                if bez.area() != 0.0 {
                    eprintln!("THERE IS NO CONTAINED POINT?! {}", bez.to_svg());
                }
                return false;
            };
            let winding: i32 = paths.iter().map(|bez| bez.winding(contained)).sum();
            winding != 0
        })
        .collect();

    // Sort filled ahead of unfilled, smaller before larger (to simplify matching below)
    let mut ordered: Vec<_> = (0..shapes.len()).collect();
    ordered.sort_by_cached_key(|i| {
        (
            -(filled[*i] as i32),
            OrderedFloat(paths[*i].bounding_box().area()),
        )
    });

    // Group cutouts with the smallest containing filled subpath
    // Doesn't generalize but perhaps suffices for icons
    // In each group [0] must exist and is a filled subpath, [1..n] are optional and are unfilled
    let mut groups: Vec<Vec<AnyShape>> = Default::default();
    let mut bboxes = Vec::default(); // the bbox of group[n][0] is bbox[n]
    for i in ordered {
        let bez = &paths[i];
        let shape = &shapes[i];
        let bbox = bez.bounding_box();
        if filled[i] {
            // start a new group for a filled subpath
            groups.push(vec![shape.clone()]);
            bboxes.push(bbox);
        } else {
            // add cutout to the smallest (first, courtesy of sort above) containing filled subpath
            if let Some(i) = bboxes
                .iter()
                .position(|group_bbox| group_bbox.intersect(bbox) == bbox)
            {
                groups[i].push(shape.clone());
            } else {
                eprintln!(
                    "Uh oh, we have an unfilled shape that didn't land anywhere! {}",
                    bez.to_svg()
                );
            }
        }
    }

    groups
}

fn nth_group_color(n: usize) -> (u8, u8, u8) {
    // Taken from https://m2.material.io/design/color/the-color-system.html#tools-for-picking-colors
    // "2014 Material Design color palettes"
    const COLORS: &[(u8, u8, u8)] = &[
        (0xEF, 0x53, 0x50),
        (0xEC, 0x40, 0x7A),
        (0xAB, 0x47, 0xBC),
        (0xE5, 0x39, 0x35),
        (0xD8, 0x1B, 0x60),
        (0x8E, 0x24, 0xAA),
        (0xC6, 0x28, 0x28),
        (0xAD, 0x14, 0x57),
        (0x6A, 0x1B, 0x9A),
    ];

    COLORS[n % COLORS.len()]
}

fn group_with_transform(shape_idx: usize, shapes: Vec<AnyShape>, transform: Transform) -> AnyShape {
    // https://lottiefiles.github.io/lottie-docs/breakdown/bouncy_ball/#transform
    // says players like to find a transform at the end of a group and having a fill before
    // the transform seems fairly ubiquotous so we'll build our pulse as a group
    // of [shapes, fill, animated transform]
    let mut group = Group::default();
    group.items.extend(shapes);

    let (r, g, b) = nth_group_color(shape_idx);

    group.items.push(AnyShape::Fill(Fill {
        opacity: Property {
            value: Value::Fixed(100.0), // default of 0 is not helpful
            ..Default::default()
        },
        color: Property {
            value: Value::Fixed(vec![r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0]), // handy for debugging grouping
            ..Default::default()
        },
        ..Default::default()
    }));
    group.items.push(AnyShape::Transform(transform));
    AnyShape::Group(group)
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

fn center(shapes: &[AnyShape]) -> Point {
    shapes
        .iter()
        .filter_map(|s| s.bounding_box())
        .reduce(|acc, e| acc.union(e))
        .map(|b| b.center())
        .unwrap_or_default()
}

fn pulse(start: f64, end: f64, shape_idx: usize, shapes: Vec<AnyShape>) -> AnyShape {
    assert!(end > start);

    let i = shape_idx as f64;
    let mut transform = Transform::default();

    // pulse around the center of the shape(s)
    // https://lottiefiles.github.io/lottie-docs/concepts/#transform
    // notes that anchor and position need to match for this
    let center = center(&shapes);
    transform.anchor_point = Property {
        value: Value::Fixed(vec![center.x, center.y]),
        ..Default::default()
    };
    transform.position = transform.anchor_point.clone();

    transform.scale.animated = 1;

    let ease = default_ease();
    transform.scale.value = Value::Animated(vec![
        MultiDimensionalKeyframe {
            start_time: 0.2 * (end - start) * i,
            start_value: Some(vec![100.0, 100.0]),
            bezier: Some(ease.clone()),
            ..Default::default()
        },
        MultiDimensionalKeyframe {
            start_time: 0.2 * (end - start) * (i + 1.0),
            start_value: Some(vec![150.0, 150.0]),
            bezier: Some(ease.clone()),
            ..Default::default()
        },
        MultiDimensionalKeyframe {
            start_time: 0.2 * (end - start) * (i + 2.0),
            start_value: Some(vec![100.0, 100.0]),
            bezier: Some(ease),
            ..Default::default()
        },
    ]);
    group_with_transform(shape_idx, shapes, transform)
}

fn twirl(start: f64, end: f64, shape_idx: usize, shapes: Vec<AnyShape>) -> AnyShape {
    assert!(end > start);

    let i = shape_idx as f64;
    let mut transform = Transform::default();

    // spin around the center of the shape(s)
    // https://lottiefiles.github.io/lottie-docs/concepts/#transform
    // notes that anchor and position need to match for this
    let center = center(&shapes);
    transform.anchor_point = Property {
        value: Value::Fixed(vec![center.x, center.y]),
        ..Default::default()
    };
    transform.position = transform.anchor_point.clone();

    transform.rotation.animated = 1;
    let ease = default_ease();
    transform.rotation.value = Value::Animated(vec![
        MultiDimensionalKeyframe {
            start_time: 0.2 * (end - start) * i,
            start_value: Some(vec![0.0]),
            bezier: Some(ease.clone()),
            ..Default::default()
        },
        MultiDimensionalKeyframe {
            start_time: 0.2 * (end - start) * (i + 2.0),
            start_value: Some(vec![360.0]),
            bezier: Some(ease),
            ..Default::default()
        },
    ]);
    group_with_transform(shape_idx, shapes, transform)
}
