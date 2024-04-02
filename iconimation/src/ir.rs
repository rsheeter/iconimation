//! An intermediate model of simple animation that can be converted to a playback format

use std::collections::HashSet;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape as KShape, Vec2};
use ordered_float::OrderedFloat;
use skrifa::{
    instance::{Location, Size},
    outline::DrawSettings,
    raw::TableProvider,
    GlyphId, OutlineGlyph,
};
use write_fonts::pens::{BezPathPen, TransformPen};

use crate::{
    bezop::{y_up_to_y_down, ContainedPoint},
    error::AnimationError,
    plan::AnimationPlan,
    GlyphShape,
};

/// A single distinct animation in a rectangular space starting at (0,0) and extending to (width, height).
/// Y-down. Timing expressed in frames which can be converted to time using frame_rate.
#[derive(Debug, Clone)]
pub struct Animation {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) frames: f64,
    pub(crate) frame_rate: f64,
    pub(crate) root: Group,
    #[allow(unused)]
    pub(crate) src_to_dest_units: Affine,
}

impl Animation {
    /// Rigs an animation to handle a Google-style icon font glyph
    pub fn of_icon(plan: &AnimationPlan, glyph_shape: &GlyphShape) -> Result<Self, AnimationError> {
        let upem = glyph_shape
            .font
            .head()
            .map_err(AnimationError::NoHeadTable)?
            .units_per_em() as f64;
        let upem_box = Rect::new(0.0, 0.0, upem, upem);
        let src_to_dest_units = y_up_to_y_down(upem_box, upem_box);

        let mut animation = Self {
            width: upem,
            height: upem,
            frames: 60.0,
            frame_rate: 60.0,
            root: Group::default(),
            src_to_dest_units,
        };
        let mut root = Group {
            center: (upem / 2.0, upem / 2.0).into(),
            ..Default::default()
        };
        root.children
            .push(Element::Shape(Keyframed::<BezPath>::for_glyph(
                animation.frames,
                src_to_dest_units,
                glyph_shape,
            )?));
        root.animate(&animation, plan);
        animation.root = root;

        Ok(animation)
    }
}

/// Create something form [`Animation`], typically an output format
pub trait FromAnimation
where
    Self: Sized,
{
    type Err;

    // Required method
    fn from_animation(s: &Animation) -> Result<Self, Self::Err>;
}

/// A set of groups or shapes that animate as one
///
/// Only element permitted transform-based animation and definition of fill
///
/// Transformation is given in terms of position, scale, and rotation around an anchor
/// because expressing rotate around point in affine form is tiresome.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub(crate) children: Vec<Element>,
    pub(crate) center: Point,
    pub(crate) fill: Option<(u8, u8, u8)>,
    pub(crate) translate: Keyframed<Vec2>,
    pub(crate) scale: Keyframed<(f64, f64)>,
    pub(crate) rotate: Keyframed<f64>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            children: Default::default(),
            center: Point::default(),
            fill: None,
            translate: Keyframed::new(0.0, Vec2::default()),
            scale: Keyframed::new(0.0, (100.0, 100.0)),
            rotate: Keyframed::new(0.0, 0.0),
        }
    }
}

impl Group {
    fn animate(&mut self, container: &Animation, plan: &AnimationPlan) {
        // Variation is apply when creating a shape; here apply transform-based animation
        match plan {
            AnimationPlan::None(..) => (),
            AnimationPlan::TwirlWhole(..) => self.rotate = twirl(0.0, container.frames, 0),
            AnimationPlan::TwirlParts(..) => {
                self.group_parts();
                for (i, g) in self.mutable_child_groups().enumerate() {
                    g.rotate = twirl(0.0, container.frames, i);
                }
            }
            AnimationPlan::PulseWhole(..) => self.scale = pulse(0.0, container.frames, 0),
            AnimationPlan::PulseParts(..) => {
                self.group_parts();
                for (i, g) in self.mutable_child_groups().enumerate() {
                    g.scale = pulse(0.0, container.frames, i);
                }
            }
            _ => todo!("Not implemented: {plan:?}"),
        }
    }

    fn mutable_child_groups(&mut self) -> impl Iterator<Item = &mut Group> {
        self.children.iter_mut().filter_map(|e| match e {
            Element::Group(g) => Some(g),
            Element::Shape(..) => None,
        })
    }
}

/// Produces keyframes suitable for use with [`Group::rotate`]
fn twirl(start: f64, end: f64, nth_group: usize) -> Keyframed<f64> {
    assert!(end > start);
    let nth_group = nth_group as f64;
    vec![
        (0.2 * (end - start) * nth_group, 0.0),
        (0.2 * (end - start) * (nth_group + 2.0), 360.0),
    ]
    .try_into()
    .unwrap()
}

/// Produces keyframes suitable for use with [`Group::scale`]
fn pulse(start: f64, end: f64, nth_group: usize) -> Keyframed<(f64, f64)> {
    assert!(end > start);
    let nth_group = nth_group as f64;
    vec![
        (0.2 * (end - start) * nth_group, (100.0, 100.0)),
        (0.2 * (end - start) * (nth_group + 1.0), (150.0, 150.0)),
        (0.2 * (end - start) * (nth_group + 2.0), (100.0, 100.0)),
    ]
    .try_into()
    .unwrap()
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
fn group_parts(shapes: Vec<Keyframed<BezPath>>) -> Vec<Group> {
    // group on subpaths; input may have multi-subpath beziers
    let shapes: Vec<_> = shapes.into_iter().flat_map(|s| s.subpaths()).collect();

    let paths: Vec<_> = shapes.iter().map(|s| &s.earliest().value).collect();

    // Figure out what is/isn't filled
    let filled: Vec<_> = paths
        .iter()
        .map(|bez| {
            let Some(contained) = bez.contained_point() else {
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
    let mut groups: Vec<Vec<Keyframed<BezPath>>> = Default::default();
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
        .into_iter()
        .enumerate()
        .map(|(i, shapes)| {
            let rgb = nth_group_color(i);
            let center = shapes
                .iter()
                .map(|k| k.earliest().value.bounding_box())
                .reduce(|acc, e| acc.union(e))
                .unwrap() // keyframed must have an entry
                .center();
            Group {
                children: shapes.into_iter().map(Element::Shape).collect(),
                center,
                fill: Some(rgb),
                ..Default::default()
            }
        })
        .collect()
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

impl Group {
    /// Piece-wise animation wants to animate "parts" as the eye perceives them; try to so group.
    pub(crate) fn group_parts(&mut self) {
        let mut frontier = vec![self];
        while let Some(group) = frontier.pop() {
            let mut new_children = Vec::new();
            let mut existing_groups = HashSet::new();
            while !group.children.is_empty() {
                // TODO: existing groups => frontier
                // runs of shapes => group_parts

                match group.children.remove(0) {
                    Element::Group(g) => {
                        existing_groups.insert(new_children.len());
                        new_children.push(Element::Group(g));
                    }
                    Element::Shape(s) => {
                        let mut shape_run = vec![s];
                        while matches!(group.children.first(), Some(Element::Shape(..))) {
                            let Element::Shape(s) = group.children.remove(0) else {
                                panic!("We just confirmed this to be the case!");
                            };
                            shape_run.push(s);
                        }
                        let groups = group_parts(shape_run);
                        new_children.extend(groups.into_iter().map(Element::Group));
                    }
                }
            }
            group.children = new_children;

            for (i, el) in group.children.iter_mut().enumerate() {
                if existing_groups.contains(&i) {
                    let Element::Group(g) = el else {
                        unreachable!();
                    };
                    frontier.push(g);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Element {
    #[allow(unused)]
    Group(Group),
    Shape(Keyframed<BezPath>),
}

/// Something with keyframes. Must have at least one definition.
///
/// Contains (f64, T) tuples sorted by .0 where the f64 is time in seconds. Times must be unique.
///
/// Pops into existence at min(time), disappears at max(time).
#[derive(Debug, Clone)]
pub struct Keyframed<T>(Vec<Keyframe<T>>);

impl<T> Keyframed<T> {
    pub(crate) fn new(frame: f64, value: T) -> Self {
        Self(vec![Keyframe::new(frame, value)])
    }

    pub(crate) fn earliest(&self) -> &Keyframe<T> {
        &self.0[0]
    }

    pub(crate) fn is_animated(&self) -> bool {
        self.len() > 1
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Keyframe<T>> {
        self.0.iter()
    }

    pub(crate) fn push(&mut self, keyframe: Keyframe<T>) {
        if let Some(pos) = self.0.iter().position(|kf| kf.frame == keyframe.frame) {
            self.0[pos] = keyframe;
        } else {
            self.0.push(keyframe);
        }
    }
}

impl<T> TryFrom<Vec<(f64, T)>> for Keyframed<T> {
    type Error = AnimationError;

    fn try_from(value: Vec<(f64, T)>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(AnimationError::NoKeyframes);
        }
        let mut value = value;
        value.sort_by_key(|(frame, _)| OrderedFloat(*frame));
        for i in 0..value.len() - 1 {
            if value[i].0 == value[i + 1].0 {
                return Err(AnimationError::MultipleValuesForFrame(value[i].0));
            }
        }
        Ok(Keyframed(
            value
                .into_iter()
                .map(|(frame, value)| Keyframe::new(frame, value))
                .collect(),
        ))
    }
}

fn draw(
    src_to_dest_units: Affine,
    location: &Location,
    gid: GlyphId,
    glyph: &OutlineGlyph,
) -> Result<BezPath, AnimationError> {
    let mut bez_pen = BezPathPen::new();
    let mut transform_pen = TransformPen::new(&mut bez_pen, src_to_dest_units);

    let settings = DrawSettings::unhinted(Size::unscaled(), location);
    glyph
        .draw(settings, &mut transform_pen)
        .map_err(|e| AnimationError::DrawError(gid, e))?;
    Ok(bez_pen.into_inner())
}

impl Keyframed<BezPath> {
    pub(crate) fn for_glyph(
        last_frame: f64,
        src_to_dest_units: Affine,
        glyph_shape: &GlyphShape,
    ) -> Result<Self, AnimationError> {
        let mut result = Self::new(
            0.0,
            draw(
                src_to_dest_units,
                &glyph_shape.start,
                glyph_shape.gid,
                &glyph_shape.glyph,
            )?,
        );

        if let Some(location) = &glyph_shape.end {
            result.push(Keyframe::new(
                last_frame,
                draw(
                    src_to_dest_units,
                    location,
                    glyph_shape.gid,
                    &glyph_shape.glyph,
                )?,
            ));
        }

        Ok(result)
    }

    pub(crate) fn subpaths(&self) -> Vec<Keyframed<BezPath>> {
        // convert each keyframe to subpaths then line 'em up
        let subpaths: Vec<_> = self.0.iter().map(|s| (s.frame, s.subpaths())).collect();

        // TODO: should we allow incompatible paths in?
        assert!(
            subpaths.iter().all(|s| s.1.len() == subpaths[0].1.len()),
            "Incompatible subpaths unsupported"
        );

        (0..subpaths[0].1.len())
            .map(|i| {
                Keyframed(
                    subpaths
                        .iter()
                        .map(|(frame, subpaths)| Keyframe::new(*frame, subpaths[i].clone()))
                        .collect(),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Keyframe<T> {
    pub frame: f64,
    pub value: T,
}

impl<T> Keyframe<T> {
    pub fn new(frame: f64, value: T) -> Self {
        Self { frame, value }
    }
}

impl Keyframe<BezPath> {
    pub(crate) fn subpaths(&self) -> Vec<BezPath> {
        let mut paths = Vec::new();
        let mut last_start = 0;
        let elements = self.value.elements();
        for (i, e) in elements.iter().enumerate().skip(1) {
            if let PathEl::MoveTo(..) = e {
                paths.push(BezPath::from_vec(elements[last_start..i].to_vec()));
                last_start = i;
            }
        }
        if last_start < elements.len() - 1 {
            paths.push(BezPath::from_vec(elements[last_start..].to_vec()));
        }
        paths
    }
}
