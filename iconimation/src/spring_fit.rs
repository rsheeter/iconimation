//! Fit a bezier to a springed animation.
//!
//! Intended use is to convert a spring animation to bezier ease(s) for environments that
//! don't have spring, e.g. native Lottie players or css-only web animation.

use kurbo::{fit_to_bezpath, BezPath, CurveFitSample, ParamCurveFit, Point, Vec2};

use crate::{
    error::SpringFitError,
    spring::{AnimatedValue, Spring},
};

struct SpringFitter {
    spring: Spring,
    frames: Vec<AnimatedValue>,
    frame_rate: f64,
    last_frame: f64,
}

impl SpringFitter {
    pub fn new(
        spring: Spring,
        animation: AnimatedValue,
        frame_rate: f64,
    ) -> Result<Self, SpringFitError> {
        let mut current = animation;
        let mut frame = 0.0;
        let mut frame_values = vec![animation];
        while !current.is_at_equilibrium() {
            frame += 1.0;
            let time = frame / frame_rate;
            current = spring.update(time, current);
            frame_values.push(current);
            if time > Self::IMPLAUSIBLY_LONG_TIME {
                return Err(SpringFitError::NoEquilibrium(time));
            }
        }

        if frame < 1.0 {
            return Err(SpringFitError::ImmediateEquilibrium);
        }
        Ok(Self {
            spring,
            frames: frame_values,
            frame_rate,
            last_frame: frame,
        })
    }

    /// (frame, value) point for the specified frame
    fn frame_value(&self, frame: f64) -> Point {
        let frame_before = frame.floor();

        let value = if frame < Self::TANGENT_FRAME_OFFSET {
            *self.frames.first().unwrap()
        } else if frame + Self::TANGENT_FRAME_OFFSET > self.last_frame {
            *self.frames.last().unwrap()
        } else if (frame - frame_before).abs() < Self::FRAME_EPSILON {
            // just take the exact value
            self.frames[frame_before as usize]
        } else {
            // Try to advance from frame before to target
            let frame_before: AnimatedValue = self.frames[frame_before as usize];
            self.spring.update(frame / self.frame_rate, frame_before)
        };
        (frame, value.value).into()
    }

    const IMPLAUSIBLY_LONG_TIME: f64 = 300.0;
    const TANGENT_FRAME_OFFSET: f64 = 0.05;
    const FRAME_EPSILON: f64 = 0.001;
}

/// In a simple spring from A => B there are no cusps which simplifies things
impl ParamCurveFit for SpringFitter {
    fn sample_pt_tangent(&self, t: f64, _sign: f64) -> CurveFitSample {
        let (p, tangent) = self.sample_pt_deriv(t);
        CurveFitSample { p, tangent }
    }

    fn sample_pt_deriv(&self, t: f64) -> (Point, Vec2) {
        let frame = t * self.last_frame;

        // Calculate prev, curr, next
        let prev = self.frame_value(frame - Self::TANGENT_FRAME_OFFSET);
        let curr = self.frame_value(frame);
        let next = self.frame_value(frame + Self::TANGENT_FRAME_OFFSET);

        // average prev->curr and curr->next
        let deriv = ((curr - prev) + (next - curr)) / 2.0;
        (curr, deriv)
    }

    fn break_cusp(&self, _range: std::ops::Range<f64>) -> Option<f64> {
        None
    }
}

/// Produce a bezier equivalent to animating the provided value with a spring until it hits equilibrium.
pub fn spring_to_bezier(
    spring: Spring,
    animation: AnimatedValue,
    frame_rate: f64,
) -> Result<BezPath, SpringFitError> {
    let fitter = SpringFitter::new(spring, animation, frame_rate)?;
    Ok(fit_to_bezpath(&fitter, 0.1))
}

#[cfg(test)]
mod tests {
    use kurbo::ParamCurveFit;
    use kurbo::PathEl;
    use kurbo::Point;

    use crate::spring::AnimatedValueType;
    use crate::spring_fit::SpringFitter;

    use super::spring_to_bezier;
    use super::AnimatedValue;
    use super::Spring;

    fn round(p: &mut Point, digits: u32) {
        let factor = 10u32.pow(digits) as f64;
        p.x = (p.x * factor).round() / factor;
        p.y = (p.y * factor).round() / factor;
    }

    #[test]
    fn spring_draw_tangents() {
        let spring = Spring::expressive_spatial();
        let animated_value = AnimatedValue::new(0.0, 100.0, AnimatedValueType::Scale);
        let fitter = SpringFitter::new(spring, animated_value, 60.0).unwrap();

        let mut frame = 0.0;
        let mut last = animated_value;
        while frame < fitter.last_frame {
            last = spring.update(frame / fitter.frame_rate, last);
            let (p, tan) = fitter.sample_pt_deriv(frame / fitter.last_frame);
            let tan = tan.normalize() * 2.0;
            let p0 = p;
            let p1 = p + tan;
            eprintln!("<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"blue\" stroke-width=\"0.2\" stroke-opacity=\"50%\" />",
                p0.x, p0.y, p1.x, p1.y);
            frame += 0.5;
        }

        let mut frame = 0.0;
        let mut last = animated_value;
        while frame < fitter.last_frame {
            last = spring.update(frame / fitter.frame_rate, last);
            eprintln!(
                "<circle cx=\"{frame:.2}\" cy=\"{:.2}\" r=\"0.5\" />",
                last.value
            );
            frame += 1.0;
        }
    }

    #[test]
    fn spring_to_bezier_scale_0_to_100() {
        let spring = Spring::expressive_spatial();
        let animated_value = AnimatedValue::new(0.0, 100.0, AnimatedValueType::Scale);

        // Let's scale from nothing to something at 60fps
        let mut bez = spring_to_bezier(spring, animated_value, 60.0).unwrap();
        for el in bez.elements_mut() {
            match el {
                PathEl::ClosePath => (),
                PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                    round(p, 2);
                }
                PathEl::QuadTo(c, p) => {
                    round(c, 2);
                    round(p, 2);
                }
                PathEl::CurveTo(c1, c2, p) => {
                    round(c1, 2);
                    round(c2, 2);
                    round(p, 2);
                }
            }
        }
        eprintln!("{}", bez.to_svg().replace(" C", "\nC"));
    }
}
