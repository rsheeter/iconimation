//! Shove glyphs from a variable font into a Lottie template.

pub mod animate;
pub mod debug_pen;
pub mod error;
mod shape_pen;

use std::f64::consts::PI;

use bodymovin::{
    layers::{AnyLayer, Layer, ShapeMixin},
    properties::{MultiDimensionalKeyframe, Property, Value},
    shapes::{AnyShape, Group, SubPath},
    sources::Asset,
    Bodymovin as Lottie,
};
use kurbo::{Affine, BezPath, Rect};
use ordered_float::OrderedFloat;
use skrifa::{instance::Size, OutlineGlyph};
use write_fonts::pens::TransformPen;

use crate::{animate::Animator, error::Error, shape_pen::SubPathPen};

pub fn default_template(font_drawbox: &Rect) -> Lottie {
    Lottie {
        in_point: 0.0,
        out_point: 60.0, // 60fps total animation = 1s
        frame_rate: 60.0,
        width: font_drawbox.width() as i64,
        height: font_drawbox.height() as i64,
        layers: vec![AnyLayer::Shape(bodymovin::layers::Shape {
            in_point: 0.0,
            out_point: 60.0, // 60fps total animation = 1s
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

pub trait Template {
    fn replace_shape(
        &mut self,
        font_drawbox: &Rect,
        glyph: &OutlineGlyph,
        animator: &dyn Animator,
    ) -> Result<(), Error>;
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

fn replace_placeholders(
    layers: &mut [AnyLayer],
    font_drawbox: &Rect,
    glyph: &OutlineGlyph,
    animator: &dyn Animator,
) -> Result<usize, Error> {
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
                let font_to_lottie = font_units_to_lottie_units(font_drawbox, &lottie_box);
                insert_at.push((i, font_to_lottie));
            }
            // reverse because replacing 1:n shifts indices past our own
            for (i, transform) in insert_at.iter().rev() {
                eprintln!("Replace {} using {:?}", shapes_updated + i, transform);
                let mut glyph_shapes: Vec<_> = subpaths_for_glyph(glyph, *transform)?;
                glyph_shapes.sort_by_cached_key(|(b, _)| {
                    let bbox = b.control_box();
                    (
                        (bbox.min_y() * 1000.0) as i64,
                        (bbox.min_x() * 1000.0) as i64,
                    )
                });
                eprintln!("Animating {} glyph shapes", glyph_shapes.len());
                let animated_shapes = animator.animate(start, end, glyph_shapes)?;
                placeholder.items.splice(*i..(*i + 1), animated_shapes);
            }
            shapes_updated += insert_at.len();
        }
    }
    Ok(shapes_updated)
}

impl Template for Lottie {
    fn replace_shape(
        &mut self,
        font_drawbox: &Rect,
        glyph: &OutlineGlyph,
        animator: &dyn Animator,
    ) -> Result<(), Error> {
        let mut shapes_updated =
            replace_placeholders(&mut self.layers, font_drawbox, glyph, animator)?;
        for asset in self.assets.iter_mut() {
            shapes_updated += match asset {
                Asset::PreComp(precomp) => {
                    replace_placeholders(&mut precomp.layers, font_drawbox, glyph, animator)?
                }
                Asset::Image(..) => 0,
            }
        }
        if shapes_updated == 0 {
            return Err(Error::NoShapesUpdated);
        }
        Ok(())
    }
}

/// Simplified version of [Affine2D::rect_to_rect](https://github.com/googlefonts/picosvg/blob/a0bcfade7a60cbd6f47d8bfe65b6d471cee628c0/src/picosvg/svg_transform.py#L216-L263)
fn font_units_to_lottie_units(font_box: &Rect, lottie_box: &Rect) -> Affine {
    assert!(font_box.width() > 0.0);
    assert!(font_box.height() > 0.0);
    assert!(lottie_box.width() > 0.0);
    assert!(lottie_box.height() > 0.0);

    let (sx, sy) = (
        lottie_box.width() / font_box.width(),
        lottie_box.height() / font_box.height(),
    );
    let transform = Affine::IDENTITY
        // Move the font box to touch the origin
        .then_translate((-font_box.min_x(), -font_box.min_y()).into())
        // Do a flip!
        .then_scale_non_uniform(1.0, -1.0)
        // Scale to match the target box
        .then_scale_non_uniform(sx, sy);

    // Line up
    let adjusted_font_box = transform.transform_rect_bbox(*font_box);
    transform.then_translate(
        (
            lottie_box.min_x() - adjusted_font_box.min_x(),
            lottie_box.min_y() - adjusted_font_box.min_y(),
        )
            .into(),
    )
}

fn bez_for_subpath(subpath: &SubPath) -> BezPath {
    let Value::Fixed(value) = &subpath.vertices.value else {
        panic!("what is {subpath:?}");
    };

    let mut path = BezPath::new();
    if !value.vertices.is_empty() {
        path.move_to(value.vertices[0]);
    }
    for (start_end, (c0, c1)) in value
        .vertices
        .windows(2)
        .zip(value.in_point.iter().zip(value.out_point.iter()))
    {
        let end = start_end[1];
        path.curve_to(*c0, *c1, end);
    }
    path
}

/// Returns a [SubPath] and [BezPath] in Lottie units for each subpath of a glyph
fn subpaths_for_glyph(
    glyph: &OutlineGlyph,
    font_units_to_lottie_units: Affine,
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
    glyph
        .draw(Size::unscaled(), &mut transform_pen)
        .map_err(Error::DrawError)?;

    Ok(subpath_pen.into_shapes())
}

/// Spring params, Compose style
pub struct AndroidSpring {
    mass: f64,
    stiffness: f64,
    damping: f64,
    initial_velocity: f64,
}

impl Default for AndroidSpring {
    fn default() -> Self {
        Self {
            mass: 1.0,
            stiffness: 100.0,
            damping: 10.0,
            initial_velocity: 0.0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Spring {
    initial_velocity: f64,
    w0: f64,
    zeta: f64,
}

impl Spring {
    fn progress(&self, progress: f64) -> f64 {
        let a = 1.0;
        // If damping is too low do things differently
        if self.zeta < 1.0 {
            let wd = self.w0 * (1.0 - self.zeta * self.zeta);
            let b = (self.zeta * self.w0 - self.initial_velocity) / wd;
            1.0 - (-progress * self.zeta * self.w0)
                * (a * (wd * progress).cos() + b * (wd * progress).sin()).exp()
        } else {
            let b = -self.initial_velocity + self.w0;
            1.0 - (a + b * progress) * (-progress * self.w0).exp()
        }
    }
}

impl Default for Spring {
    fn default() -> Self {
        AndroidSpring::default().into()
    }
}

impl From<AndroidSpring> for Spring {
    fn from(value: AndroidSpring) -> Self {
        let mass = 1.0;
        let mult = 2.0 * PI / (value.stiffness / value.mass).sqrt();
        let damping = 4.0 * PI * value.damping * mass / mult;
        Spring {
            initial_velocity: value.initial_velocity,
            w0: (value.stiffness / mass).sqrt(),
            zeta: damping / (2.0 * (value.stiffness * mass).sqrt()),
        }
    }
}

/// Move between keyframes using a spring. Boing.
pub trait SpringBetween {
    /// Populate the motion between keyframes using a spring function    
    fn spring(&mut self, spring: Spring) -> Result<(), Error>;
}

impl SpringBetween for Vec<MultiDimensionalKeyframe> {
    fn spring(&mut self, spring: Spring) -> Result<(), Error> {
        // Sort so windows yield consecutive keyframes in time
        self.sort_by_key(|k| OrderedFloat(k.start_time));
        let mut new_frames = Vec::new();
        for i in 0..self.len() - 1 {
            let [k0, k1] = &self[i..i + 2] else {
                panic!("Illegal state");
            };
            let Some(start_values) = &k0.start_value else {
                continue;
            };
            let Some(end_values) = &k1.start_value else {
                continue;
            };

            let start = k0.start_time.ceil() as usize;
            let end = k1.start_time.floor() as usize;
            if start + 1 >= end {
                eprintln!("Nop interpolate {} to {}", k0.start_time, k1.start_time);
                continue;
            }

            eprintln!("Generate frames {} to {}", start + 1, end - 1);
            for i in start + 1..end {
                let progress = (i - start) as f64 / (end - start) as f64;
                let sprung = spring.progress(progress);
                let mut new_frame = (*k0).clone();
                new_frame.start_time = i as f64;
                new_frame.start_value = Some(
                    start_values
                        .iter()
                        .zip(end_values)
                        .map(|(start, end)| (*end - *start) * sprung + start)
                        .collect(),
                );
                new_frames.push(new_frame);
                //eprintln!("  {i} {progress:.2} {sprung:.2}");
            }
        }
        if new_frames.is_empty() {
            return Err(Error::NoTransformsUpdated);
        }

        eprintln!(
            "Had {} frames, now {}",
            self.len(),
            self.len() + new_frames.len()
        );
        self.extend(new_frames);
        self.sort_by_key(|k| OrderedFloat(k.start_time));

        Ok(())
    }
}

/// Position and scale look like this
impl SpringBetween for Property<Vec<f64>, MultiDimensionalKeyframe> {
    fn spring(&mut self, spring: Spring) -> Result<(), Error> {
        let Value::Animated(keyframes) = &mut self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.spring(spring)
    }
}

/// Rotation looks like this
impl SpringBetween for Property<f64, MultiDimensionalKeyframe> {
    fn spring(&mut self, spring: Spring) -> Result<(), Error> {
        let Value::Animated(keyframes) = &mut self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.spring(spring)
    }
}

impl SpringBetween for Lottie {
    fn spring(&mut self, spring: Spring) -> Result<(), Error> {
        let mut transforms_updated = 0;
        for layer in self.layers.iter_mut() {
            let AnyLayer::Shape(layer) = layer else {
                continue;
            };
            let placeholders = placeholders(layer);
            for placeholder in placeholders {
                let Some(AnyShape::Transform(transform)) = placeholder.items.last_mut() else {
                    eprintln!("A placeholder without a transform last?!");
                    continue;
                };
                for result in [
                    transform.scale.spring(spring),
                    transform.position.spring(spring),
                    transform.rotation.spring(spring),
                ] {
                    match result {
                        Ok(()) => transforms_updated += 1,
                        Err(Error::NoTransformsUpdated) => (),
                        Err(..) => return result,
                    }
                }
            }
        }
        if transforms_updated == 0 {
            return Err(Error::NoTransformsUpdated);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {}
