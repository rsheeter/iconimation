//! Shove glyphs from a variable font into a Lottie template.

pub mod animate;
pub mod debug_pen;
pub mod error;
mod shape_pen;
pub mod spring;
pub mod spring_fit;

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

    fn spring(&mut self, spring: Spring) -> Result<(), Error> {
        let timing = Timing::new(self);
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
                    transform.scale.spring(timing, spring),
                    transform.position.spring(timing, spring),
                    transform.rotation.spring(timing, spring),
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
    pub mass: f64,
    pub stiffness: f64,
    pub damping: f64,
    pub initial_velocity: f64,
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
struct Timing {
    in_point: f64,
    out_point: f64,
    frame_rate: f64,
}

impl Timing {
    fn new(lottie: &Lottie) -> Self {
        Timing {
            in_point: lottie.in_point,
            out_point: lottie.out_point,
            frame_rate: lottie.frame_rate,
        }
    }

    fn frame_to_time(&self, frame: f64) -> f64 {
        frame / self.frame_rate
    }

    /// Assumes keyframes are sorted by time.
    ///
    /// getMostRecentKeyIndex in js version
    fn keyframe_before(&self, time: f64, keyframes: &[MultiDimensionalKeyframe]) -> Option<usize> {
        let mut result = None;
        for (i, keyframe) in keyframes.iter().enumerate() {
            if time >= self.frame_to_time(keyframe.start_time) {
                result = Some(i);
            }
        }
        result
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Spring {
    initial_velocity: f64,
    w0: f64,
    zeta: f64,
}

impl Spring {
    pub fn new(initial_velocity: f64, stiffness: f64, damping: f64) -> Spring {
        let mass = 1.0;
        Spring {
            initial_velocity,
            w0: (stiffness / mass).sqrt(),
            zeta: damping / (2.0 * (stiffness * mass).sqrt()),
        }
    }

    /// progress how much time has elapsed since t0
    ///
    /// getSpringedProgress in js
    fn progress(&self, progress: f64) -> f64 {
        let a = 1.0;
        // If damping is too low do things differently
        if self.zeta < 1.0 {
            let wd = self.w0 * (1.0 - self.zeta * self.zeta).sqrt();
            let b = (self.zeta * self.w0 - self.initial_velocity) / wd;
            1.0 - (-progress * self.zeta * self.w0).exp()
                * (a * (wd * progress).cos() + b * (wd * progress).sin())
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
        Spring::new(value.initial_velocity, value.stiffness, damping)
    }
}

/// Move between keyframes using a spring. Boing.
trait SpringBetween {
    type Item;

    fn springed_value(
        &self,
        time: f64,
        timing: Timing,
        spring: Spring,
    ) -> Result<Self::Item, Error>;

    /// Populate the motion between keyframes using a spring function    
    fn spring(&mut self, timing: Timing, spring: Spring) -> Result<(), Error>;
}

/// calculateAnimatedValue in js
fn animated_value(from: &[f64], to: &[f64], progress: f64) -> Vec<f64> {
    assert_eq!(
        from.len(),
        to.len(),
        "Nonsensical from/to, from {from:?} to {to:?}"
    );
    from.iter()
        .zip(to)
        .map(|(from, to)| {
            let delta = to - from;
            let updated_delta = delta * progress;
            from + updated_delta
        })
        .collect()
}

const KEYFRAME_VALUE_EPSILON: f64 = 0.01;

impl SpringBetween for Vec<MultiDimensionalKeyframe> {
    type Item = MultiDimensionalKeyframe;

    fn springed_value(
        &self,
        frame: f64,
        timing: Timing,
        spring: Spring,
    ) -> Result<Self::Item, Error> {
        let time = timing.frame_to_time(frame);
        let keyframe_idx = timing.keyframe_before(time, self).unwrap_or(0);

        // getKeys in js
        let k0 = &self[keyframe_idx.saturating_sub(1)];
        let k1 = &self[keyframe_idx];

        let springed_progress = spring.progress(time - timing.frame_to_time(k1.start_time));
        let end_values = k1.start_value.as_ref().unwrap();

        // we want to start from where we last finished; getPrevAnimationEndValue in js
        let prev_end_value = if keyframe_idx > 0 {
            let pk0 = &self[keyframe_idx.saturating_sub(2)];
            let pk1 = &self[keyframe_idx.saturating_sub(1)];
            let progress = spring.progress(
                timing.frame_to_time(k1.start_time) - timing.frame_to_time(k0.start_time),
            );
            let prev_value = animated_value(
                pk0.start_value.as_ref().unwrap(),
                pk1.start_value.as_ref().unwrap(),
                progress,
            );
            prev_value
        } else {
            end_values.to_vec()
        };

        let mut new_frame = (*k0).clone();
        new_frame.start_time = frame;
        new_frame.start_value = Some(animated_value(
            &prev_end_value,
            end_values,
            springed_progress,
        ));

        Ok(new_frame)
    }

    fn spring(&mut self, timing: Timing, spring: Spring) -> Result<(), Error> {
        // Sort so windows yield consecutive keyframes in time
        self.sort_by_key(|k| OrderedFloat(k.start_time));
        let mut new_frames = Vec::new();

        // +50 : TEMPORARY DUE TO TESTDATA
        for frame in timing.in_point.ceil() as usize..=(timing.out_point.floor() as usize + 50) {
            let new_frame = self.springed_value(frame as f64, timing, spring);
            new_frames.push(new_frame?);
        }
        if new_frames.is_empty() {
            return Err(Error::NoTransformsUpdated);
        }

        // Our Springs tend to generate series of almost identical frames; blast those
        if new_frames.len() > 2 {
            let mut i = 0;
            let mut drops = Vec::new();
            while i < new_frames.len() - 2 {
                let Some(i_value) = &new_frames[i].start_value.as_ref() else {
                    continue;
                };
                let mut run_len: usize = 0;
                for j_frame in new_frames.iter().skip(i + 1) {
                    let Some(j_value) = j_frame.start_value.as_ref() else {
                        break;
                    };
                    if i_value
                        .iter()
                        .zip(j_value.iter())
                        .any(|(a, b)| (a - b).abs() >= KEYFRAME_VALUE_EPSILON)
                    {
                        break;
                    }
                    run_len += 1;
                }
                if run_len > 2 {
                    drops.extend(i + 1..i + run_len);
                }

                i += run_len + 1;
            }
            eprintln!("Drop {} unnecessary (duplicate) frames", drops.len());
            for i in drops.into_iter().rev() {
                new_frames.remove(i);
            }
        }

        // completely replace the original frames
        self.clear();
        self.extend(new_frames);

        Ok(())
    }
}

/// Position and scale look like this
impl SpringBetween for Property<Vec<f64>, MultiDimensionalKeyframe> {
    type Item = MultiDimensionalKeyframe;

    fn springed_value(
        &self,
        time: f64,
        timing: Timing,
        spring: Spring,
    ) -> Result<Self::Item, Error> {
        let Value::Animated(keyframes) = &self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.springed_value(time, timing, spring)
    }

    fn spring(&mut self, timing: Timing, spring: Spring) -> Result<(), Error> {
        let Value::Animated(keyframes) = &mut self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.spring(timing, spring)
    }
}

/// Rotation looks like this
impl SpringBetween for Property<f64, MultiDimensionalKeyframe> {
    type Item = MultiDimensionalKeyframe;

    fn springed_value(
        &self,
        time: f64,
        timing: Timing,
        spring: Spring,
    ) -> Result<Self::Item, Error> {
        let Value::Animated(keyframes) = &self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.springed_value(time, timing, spring)
    }

    fn spring(&mut self, timing: Timing, spring: Spring) -> Result<(), Error> {
        let Value::Animated(keyframes) = &mut self.value else {
            return Err(Error::NoTransformsUpdated);
        };
        keyframes.spring(timing, spring)
    }
}

#[cfg(test)]
mod tests {
    use crate::{AndroidSpring, Spring};

    // value at the in-point for one of our demo files which we initially computed entirely wrong courtesy of a few key mistranscriptions :)
    #[test]
    fn spring_progress_at_demo_inpoint() {
        let spring: Spring = AndroidSpring {
            damping: 0.8,
            stiffness: 380.0,
            ..Default::default()
        }
        .into();
        assert_eq!(-663.66, (100.0 * spring.progress(-0.4)).round() / 100.0);
    }
}
