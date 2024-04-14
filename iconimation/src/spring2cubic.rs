//! Hand-written cubics for Spring motion.
//!
//! As per <https://github.com/rsheeter/iconimation/pull/24>, this seems feasible to automate
//! but at time of writing hand-written curves are winning.
//!
//! <https://codepen.io/rs42/pen/JjzpPyP> shows drafting of the manual curves.

use kurbo::{Affine, CubicBez};

use crate::{
    error::CubicApproximationError,
    spring::{AnimatedValue, Spring},
};

static TIME_LIMIT: f64 = 5.0;

/// Creates cubics to approximate a spring animation.
///
/// Supports only well known springs due to <https://github.com/rsheeter/iconimation/issues/29>:
/// * [`Spring::standard`]
/// * [`Spring::smooth_spatial`]
/// * [`Spring::smooth_non_spatial`]
/// * [`Spring::expressive_spatial`]
/// * [`Spring::expressive_non_spatial`]
pub fn cubic_approximation(
    frame_rate: f64,
    animation: AnimatedValue,
    spring: Spring,
) -> Result<Vec<CubicBez>, CubicApproximationError> {
    let handwritten_curve = handwritten_cubic(spring)?;

    let num_frames = num_frames(frame_rate, animation, spring)?;

    // X is time in frames. Scale hand-written curve to match.
    let sx = num_frames as f64 / handwritten_curve.last().unwrap().p3.x;

    // Y is the actual value. Shift and scale to match.
    // Hand-written always cover 0=>100. Shift to match initial value. Scale to match range.
    let dy = animation.value;
    let sy = (animation.final_value - animation.value) / 100.0;

    let transform = Affine::scale_non_uniform(sx, sy).then_translate((0.0, dy).into());

    Ok(handwritten_curve
        .into_iter()
        .map(|c| transform * c)
        .collect())
}

fn handwritten_cubic(spring: Spring) -> Result<Vec<CubicBez>, CubicApproximationError> {
    // Hand-written curves x=frame, y=value
    // x is 0 .. frame of equilibrium, y starts at 0 and ends at 100
    // Scale to match the input animation.
    Ok(match spring {
        // Several springs are identical to standard
        _ if spring == Spring::standard() => vec![CubicBez {
            p0: (0.0, 0.0).into(),
            p1: (13.0, 100.0).into(),
            p2: (0.0, 100.0).into(),
            p3: (43.0, 100.0).into(),
        }],
        _ if spring == Spring::smooth_spatial() => vec![CubicBez {
            p0: (0.0, 0.0).into(),
            p1: (20.0, 100.0).into(),
            p2: (0.0, 100.0).into(),
            p3: (61.0, 100.0).into(),
        }],
        _ if spring == Spring::expressive_spatial() => vec![
            CubicBez {
                p0: (0.0, 0.0).into(),
                p1: (5.0, 15.0).into(),
                p2: (3.0, 101.54).into(),
                p3: (15.5, 101.54).into(),
            },
            CubicBez {
                p0: (15.5, 101.54).into(),
                p1: (21.0, 101.54).into(),
                p2: (21.0, 99.0).into(),
                p3: (42.0, 100.0).into(),
            },
        ],
        _ => return Err(CubicApproximationError::UnrecognizedSpring),
    })
}

fn num_frames(
    frame_rate: f64,
    animation: AnimatedValue,
    spring: Spring,
) -> Result<usize, CubicApproximationError> {
    // Run the specified animation to equilibrium to learn it's bounds
    let mut frame = 0;
    let mut animated_value = animation;
    while !animated_value.is_at_equilibrium() {
        let time = frame as f64 / frame_rate;
        if time > TIME_LIMIT {
            return Err(CubicApproximationError::RanTooLong);
        }
        animated_value = spring.update(time, animated_value);
        frame += 1;
    }
    Ok(frame)
}
