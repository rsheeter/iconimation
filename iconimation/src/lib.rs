//! Shove glyphs from a variable font into a Lottie template.

pub mod android;
pub mod animate_legacy;
mod bezop;
pub mod debug_pen;
pub mod error;
pub mod ir;
pub mod ligate;
pub mod lottie;
pub mod plan;
mod shape_pen;
pub mod spring;

use std::fmt::Debug;

use bezop::y_up_to_y_down;
use bodymovin::{
    helpers::Transform,
    layers::{AnyLayer, Layer, ShapeMixin},
    properties::{
        Bezier2d, BezierEase, ControlPoint2d, MultiDimensionalKeyframe, Property, ShapeKeyframe,
        ShapeValue, SplittableMultiDimensional, Value,
    },
    shapes::{AnyShape, Group, SubPath},
    sources::Asset,
    Bodymovin as Lottie,
};
use kurbo::{Affine, BezPath, PathEl, Point, Rect};
use ordered_float::OrderedFloat;
use plan::AnimationPlan;
use skrifa::{
    instance::{Location, LocationRef, Size},
    outline::DrawSettings,
    raw::{FontRef, TableProvider},
    GlyphId, MetadataProvider, OutlineGlyph,
};
use spring::{AnimatedValue, AnimatedValueType, Spring};
use write_fonts::pens::TransformPen;

use crate::{error::Error, shape_pen::SubPathPen};

pub fn generate_lottie(
    font: &FontRef,
    command: &AnimationPlan,
    glyph_shape: &GlyphShape,
) -> Result<Lottie, Error> {
    let upem = font.head().unwrap().units_per_em() as f64;
    let font_drawbox: Rect = (Point::ZERO, Point::new(upem, upem)).into();

    let mut lottie = lottie_template(&font_drawbox);
    let animation = command.legacy_animation(glyph_shape);
    lottie.replace_shape(&animation)?;
    if let Some(spring) = command.spring() {
        lottie.spring(spring)?
    }
    Ok(lottie)
}

pub fn lottie_template(font_drawbox: &Rect) -> Lottie {
    Lottie {
        in_point: 0.0,
        out_point: 60.0, // 60fps total animation = 1s
        frame_rate: 60.0,
        width: font_drawbox.width() as i64,
        height: font_drawbox.height() as i64,
        layers: vec![AnyLayer::Shape(bodymovin::layers::Shape {
            in_point: 0.0,
            out_point: 60.0, // 60fps total animation = 1s
            transform: Transform {
                position: SplittableMultiDimensional::Uniform(Property {
                    value: Value::Fixed(vec![
                        font_drawbox.width() / 2.0,
                        font_drawbox.height() / 2.0,
                    ]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            mixin: ShapeMixin {
                shapes: vec![AnyShape::Group(Group {
                    name: Some("placeholder".into()),
                    items: vec![
                        // de facto standard is shape(s), fill, transform
                        AnyShape::Rect(bodymovin::shapes::Rect {
                            position: Property {
                                value: Value::Fixed(vec![0.0, 0.0]),
                                ..Default::default()
                            },
                            size: Property {
                                value: Value::Fixed(vec![
                                    font_drawbox.width(),
                                    font_drawbox.height(),
                                ]),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        AnyShape::Fill(Default::default()),
                        AnyShape::Transform(Default::default()),
                    ],
                    ..Default::default()
                })],
                ..Default::default()
            },
            ..Default::default()
        })],
        ..Default::default()
    }
}

/// Produces things that could replace a placeholder in a Lottie [`Template`]
pub trait ToLottie: Debug {
    fn create(&self, start: f64, end: f64, dest_box: Rect) -> Result<Vec<AnyShape>, Error>;
}

pub struct GlyphShape<'a> {
    font: &'a FontRef<'a>,
    glyph: OutlineGlyph<'a>,
    gid: GlyphId,
    start: Location,
    // If set, animate from start => end
    end: Option<Location>,
}

impl<'a> Debug for GlyphShape<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphShape")
            .field("gid", &self.gid)
            .finish()
    }
}

impl<'a> GlyphShape<'a> {
    pub fn new(
        font: &'a FontRef<'a>,
        gid: GlyphId,
        start: Location,
        mut end: Option<Location>,
    ) -> Result<Self, Error> {
        let outline_loader = font.outline_glyphs();
        let Some(glyph) = outline_loader.get(gid) else {
            return Err(Error::NoOutline(gid));
        };
        if let Some(end_loc) = &end {
            if start.coords() == end_loc.coords() {
                end = None;
            }
        }
        Ok(Self {
            font,
            glyph,
            gid,
            start,
            end,
        })
    }

    pub fn drawbox(&self) -> Rect {
        let upem = self.font.head().unwrap().units_per_em() as f64;
        (Point::ZERO, Point::new(upem, upem)).into()
    }
}

fn path_commands(bez: &BezPath) -> String {
    bez.elements()
        .iter()
        .map(|e| match e {
            PathEl::ClosePath => 'Z',
            PathEl::CurveTo(..) => 'C',
            PathEl::LineTo(..) => 'L',
            PathEl::MoveTo(..) => 'M',
            PathEl::QuadTo(..) => 'Q',
        })
        .collect()
}

impl<'a> ToLottie for GlyphShape<'a> {
    fn create(&self, start: f64, end: f64, dest_box: Rect) -> Result<Vec<AnyShape>, Error> {
        let transform = y_up_to_y_down(self.drawbox(), dest_box);

        // We need at least the starting outline
        let start_loc = (&self.start).into();
        let mut start_shapes: Vec<_> = subpaths_for_glyph(&self.glyph, transform, start_loc)?;

        // TODO: sort is unsafe unless we sort all shapes consistently. And the order might change across designspace.
        // glyph_shapes.sort_by_cached_key(|(b, _)| {
        //     let bbox = b.control_box();
        //     (
        //         (bbox.min_y() * 1000.0) as i64,
        //         (bbox.min_x() * 1000.0) as i64,
        //     )
        // });

        // Maybe there is an ending outline, and if there is there might be intermediary stops too
        if let Some(end_loc) = self.end.as_ref() {
            let end_shapes: Vec<_> = subpaths_for_glyph(&self.glyph, transform, end_loc.into())?;

            let start_cmds = start_shapes
                .iter()
                .map(|(bez, _)| path_commands(bez))
                .collect::<Vec<_>>()
                .join("\n");
            let end_cmds = end_shapes
                .iter()
                .map(|(bez, _)| path_commands(bez))
                .collect::<Vec<_>>()
                .join("\n");

            // TODO: figure out where to swap shapes if start/end aren't compatible
            // In theory you could swap several times such that start and end are compatible but there are swaps between. Don't care.
            assert!(start_cmds == end_cmds);
            eprintln!(
                "OMG, we have {} start shapes and {} end shapes. Compatible? {}",
                start_shapes.len(),
                end_shapes.len(),
                start_cmds == end_cmds
            );

            // https://lottiefiles.github.io/lottie-docs/playground/json_editor/ doesn't play if there is no ease
            let ease = BezierEase::_2D(Bezier2d {
                in_value: ControlPoint2d { x: 0.6, y: 1.0 },
                out_value: ControlPoint2d { x: 0.4, y: 0.0 },
            });

            for ((_, start_path), (_, end_path)) in start_shapes.iter_mut().zip(end_shapes) {
                let (Value::Fixed(start_value), Value::Fixed(end_value)) =
                    (&start_path.vertices.value, end_path.vertices.value)
                else {
                    panic!("Subpaths should be fixed");
                };

                if *start_value == end_value {
                    continue;
                }

                eprintln!("Generating animation");
                start_path.vertices.animated = 1;
                start_path.vertices.value = Value::Animated(vec![
                    ShapeKeyframe {
                        start_time: start,
                        start_value: Some(vec![start_value.clone()]),
                        // no ease, no render
                        bezier: Some(ease.clone()),
                        ..Default::default()
                    },
                    ShapeKeyframe {
                        start_time: end,
                        start_value: Some(vec![end_value]),
                        bezier: Some(ease.clone()),
                        ..Default::default()
                    },
                ]);
            }
        }

        Ok(start_shapes
            .into_iter()
            .map(|(_, s)| AnyShape::Shape(s))
            .collect())
    }
}

pub trait Template {
    fn replace_shape(&mut self, replacer: &impl ToLottie) -> Result<(), Error>;

    fn spring(&mut self, spring: Spring) -> Result<(), Error>;
}

fn placeholders(layer: &mut Layer<ShapeMixin>) -> Vec<&mut Group> {
    layer
        .mixin
        .shapes
        .iter_mut()
        .filter_map(|any| match any {
            AnyShape::Group(group) if group.name.as_deref() == Some("placeholder") => Some(group),
            _ => None,
        })
        .collect()
}

fn replace_placeholders(layers: &mut [AnyLayer], replacer: &impl ToLottie) -> Result<usize, Error> {
    let mut shapes_updated = 0;
    for layer in layers.iter_mut() {
        let AnyLayer::Shape(layer) = layer else {
            continue;
        };
        let (start, end) = (layer.in_point, layer.out_point);
        let placeholders = placeholders(layer);

        let mut insert_at = Vec::with_capacity(1);
        for placeholder in placeholders {
            insert_at.clear();
            for (i, item) in placeholder.items.iter_mut().enumerate() {
                let lottie_box = match item {
                    AnyShape::Shape(shape) => Some(bez_for_subpath(shape).control_box()),
                    AnyShape::Rect(rect) => {
                        let Value::Fixed(pos) = &rect.position.value else {
                            panic!("Unable to process {rect:#?} position, must be fixed");
                        };
                        let Value::Fixed(size) = &rect.size.value else {
                            panic!("Unable to process {rect:#?} size, must be fixed");
                        };
                        assert_eq!(2, pos.len());
                        assert_eq!(2, size.len());
                        // https://lottiefiles.github.io/lottie-docs/schema/#/$defs/shapes/rectangle notes position
                        // of a rect is the center; what we want is top-left, bottom-right
                        let (x0, y0) = (pos[0] - size[0] / 2.0, pos[1] - size[1] / 2.0);
                        Some(Rect {
                            x0,
                            y0,
                            x1: x0 + size[0],
                            y1: y0 + size[1],
                        })
                    }
                    _ => None,
                };
                let Some(lottie_box) = lottie_box else {
                    continue;
                };
                insert_at.push((i, lottie_box));
            }
            // reverse because replacing 1:n shifts indices past our own
            for (i, dest_box) in insert_at.iter().rev() {
                let glyph_shapes = replacer.create(start, end, *dest_box)?;
                placeholder.items.splice(*i..(*i + 1), glyph_shapes);
            }
            shapes_updated += insert_at.len();
        }
    }
    Ok(shapes_updated)
}

impl Template for Lottie {
    fn replace_shape(&mut self, replacer: &impl ToLottie) -> Result<(), Error> {
        let mut shapes_updated = replace_placeholders(&mut self.layers, replacer)?;
        for asset in self.assets.iter_mut() {
            shapes_updated += match asset {
                Asset::PreComp(precomp) => replace_placeholders(&mut precomp.layers, replacer)?,
                Asset::Image(..) => 0,
            }
        }
        if shapes_updated == 0 {
            return Err(Error::NoShapesUpdated);
        }
        Ok(())
    }

    fn spring(&mut self, spring: Spring) -> Result<(), Error> {
        let mut transforms_updated = 0;
        let mut num_placeholders = 0;
        for layer in self.layers.iter_mut() {
            let AnyLayer::Shape(layer) = layer else {
                continue;
            };
            let mut frontier = placeholders(layer);
            num_placeholders += frontier.len();
            while let Some(group) = frontier.pop() {
                for item in group.items.iter_mut() {
                    match item {
                        AnyShape::Group(group) => frontier.push(group),
                        AnyShape::Transform(transform) => {
                            for result in [
                                transform.scale.spring(
                                    self.frame_rate,
                                    AnimatedValueType::Scale,
                                    spring,
                                ),
                                transform.position.spring(
                                    self.frame_rate,
                                    AnimatedValueType::Position,
                                    spring,
                                ),
                                transform.rotation.spring(
                                    self.frame_rate,
                                    AnimatedValueType::Rotation,
                                    spring,
                                ),
                            ] {
                                match result {
                                    Ok(()) => transforms_updated += 1,
                                    Err(Error::NoTransformsUpdated) => (),
                                    Err(..) => return result,
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
        if num_placeholders == 0 {
            return Err(Error::NoPlaceholders);
        }
        if transforms_updated == 0 {
            return Err(Error::NoTransformsUpdated);
        }
        Ok(())
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

pub(crate) fn bez_for_subpath(subpath: &SubPath) -> BezPath {
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

/// Returns a [SubPath] and [BezPath] in Lottie units for each subpath of a glyph
fn subpaths_for_glyph(
    glyph: &OutlineGlyph,
    font_units_to_lottie_units: Affine,
    location: LocationRef,
) -> Result<Vec<(BezPath, SubPath)>, Error> {
    // Fonts draw Y-up, Lottie Y-down. The transform to transition should be negative determinant.
    // Normally a negative determinant flips curve direction but since we're also moving
    // to a coordinate system with Y flipped it should cancel out.
    assert!(
        font_units_to_lottie_units.determinant() < 0.0,
        "We assume a negative determinant"
    );

    let mut subpath_pen = SubPathPen::default();
    let mut transform_pen = TransformPen::new(&mut subpath_pen, font_units_to_lottie_units);
    let settings = DrawSettings::unhinted(Size::unscaled(), location);
    glyph
        .draw(settings, &mut transform_pen)
        .map_err(Error::DrawError)?;

    Ok(subpath_pen.into_shapes())
}

/// Move between keyframes using a spring. Boing.
trait SpringBetween {
    type Item;

    /// Populate the motion between keyframes using a spring function    
    fn spring(
        &mut self,
        frame_rate: f64,
        value_type: AnimatedValueType,
        spring: Spring,
    ) -> Result<(), Error>;
}

impl SpringBetween for Vec<MultiDimensionalKeyframe> {
    type Item = MultiDimensionalKeyframe;

    fn spring(
        &mut self,
        frame_rate: f64,
        value_type: AnimatedValueType,
        spring: Spring,
    ) -> Result<(), Error> {
        if self.len() < 2 {
            eprintln!("Spring nop, our len is {}", self.len());
            return Ok(()); // nop w/o at least 2 keys
        }

        if self.len() != 2 {
            panic!("TODO: multiple keyframe support");
        }

        // Sort so windows yield consecutive keyframes in time
        self.sort_by_key(|k| OrderedFloat(k.start_time));

        let mut new_frames = Vec::new();

        for keyframes in self.windows(2) {
            let k0 = &keyframes[0];
            let k0_frame = k0.start_time.floor();
            let k1 = &keyframes[1];
            //let k1_frame = k1.start_time.ceil();

            let Some(start_values) = &k0.start_value else {
                continue;
            };
            let Some(end_values) = &k1.start_value else {
                continue;
            };
            if start_values.len() != end_values.len() {
                return Err(Error::ValueLengthMismatch(
                    value_type,
                    start_values.clone(),
                    end_values.clone(),
                ));
            }

            // Start a new chain of animated values for each independent value
            let mut frame_values = Vec::<Vec<AnimatedValue>>::new();
            frame_values.push(
                (0..start_values.len())
                    .map(|i| AnimatedValue::new(start_values[i], end_values[i], value_type))
                    .collect(),
            );

            // We're done when all values reach equilibrium
            // TODO: we also want to be done in the alloted time which means we need to scale the result
            while let Some(current) = frame_values.last() {
                if current.iter().all(|av| av.is_at_equilibrium()) {
                    break;
                }
                let frame = frame_values.len();
                let time = frame as f64 / frame_rate;

                let next = current
                    .iter()
                    .map(|curr| spring.update(time, *curr))
                    .collect();
                frame_values.push(next);
            }
            eprintln!("Equilibrium after {} frames", frame_values.len());

            for (frame_offset, values) in frame_values.into_iter().enumerate() {
                let mut new_frame = (*k0).clone();
                new_frame.start_time = k0_frame + frame_offset as f64;
                new_frame.start_value = Some(values.into_iter().map(|av| av.value).collect());
                new_frames.push(new_frame);
            }
        }

        // completely replace the original frames
        eprintln!(
            "Update {value_type:?} from {} to {} frames",
            self.len(),
            new_frames.len()
        );
        self.clear();
        self.extend(new_frames);

        Ok(())
    }
}

/// Position and scale look like this
impl SpringBetween for Property<Vec<f64>, MultiDimensionalKeyframe> {
    type Item = MultiDimensionalKeyframe;

    fn spring(
        &mut self,
        frame_rate: f64,
        value_type: AnimatedValueType,
        spring: Spring,
    ) -> Result<(), Error> {
        let Value::Animated(keyframes) = &mut self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.spring(frame_rate, value_type, spring)
    }
}

/// Rotation looks like this
impl SpringBetween for Property<f64, MultiDimensionalKeyframe> {
    type Item = MultiDimensionalKeyframe;

    fn spring(
        &mut self,
        frame_rate: f64,
        value_type: AnimatedValueType,
        spring: Spring,
    ) -> Result<(), Error> {
        let Value::Animated(keyframes) = &mut self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.spring(frame_rate, value_type, spring)
    }
}

#[cfg(test)]
mod tests {}
