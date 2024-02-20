//! Describes programmatic animators, produces of motion curves for transitions between values

use std::{fmt::Debug, time::Duration};

use ordered_float::OrderedFloat;

use crate::{
    animated_glyph::AnimatedGlyph,
    error::{AnimationError, ToDeliveryError},
};

/// Fraction progress within an interval.
///
/// Concretely, a value [0, 1], such as progression through some as yet unknown time interval.
/// The contained value may be outside [0, 1] because clamp and float ops aren't const-friendly
/// but it will be clamped before you get your filthy paws on it via.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct IntervalPosition(f64);

impl IntervalPosition {
    pub const START: Self = IntervalPosition(0.0);
    pub const END: Self = IntervalPosition(1.0);

    pub const fn new(pos: f64) -> Self {
        Self(pos)
    }

    pub fn into_inner(self) -> f64 {
        self.0.clamp(0.0, 1.0)
    }
}

pub struct Cubic {
    p1: (IntervalPosition, f64),
    p2: (IntervalPosition, f64),
    p3: (IntervalPosition, f64),
}

pub struct MotionCurve {
    start: (IntervalPosition, f64),
    /// If there are any cubics then he interval position of the last cubic must be 1.0
    motion: Vec<Cubic>,
}

/// A potentially animated value.
///
/// Contains keyframes for a value, sorted by IntervalPosition. Keyframes must have unique positions.
/// The spaces in between keyframes are meant to filled in based on a [`MotionCurve`]. There is always
/// at least one keyframe because allowing none makes use of the type tiresome.
#[derive(Clone, Debug)]
pub struct Animated<T>(Vec<(IntervalPosition, T)>);

impl<T> Animated<T> {
    pub fn start_only(value: T) -> Self {
        Self(vec![(IntervalPosition::START, value)])
    }

    pub fn new(
        positions: impl IntoIterator<Item = (IntervalPosition, T)>,
    ) -> Result<Self, AnimationError> {
        let mut positions: Vec<_> = positions.into_iter().collect();
        positions.sort_by_key(|(p, _)| OrderedFloat(p.0));
        for window in positions.windows(2) {
            let p0 = window[0].0;
            let p1 = window[1].0;
            if p0 == p1 {
                return Err(AnimationError::DuplicateKeyframes);
            }
        }
        if positions.is_empty() {
            return Err(AnimationError::NoKeyframes);
        }
        Ok(Self(positions))
    }

    pub fn first(&self) -> &T {
        &self.0.first().unwrap().1
    }

    pub fn iter(&self) -> impl Iterator<Item = &(IntervalPosition, T)> {
        self.0.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = (IntervalPosition, T)> {
        self.0.into_iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_animated(&self) -> bool {
        self.len() > 1
    }
}

/// A producer of motion (value over time) curves
///
/// Named in honor of ATLA and ofc Futurama
pub trait MotionBender: Debug {
    /// Produces cubic(s) describing the path between (0, start_value) and (1, end_value)
    ///
    /// Start/end value are the animated property. X-value is time expressed as progression
    /// through an unknown interval.
    fn animate_between(&self, start_value: f64, end_value: f64) -> MotionCurve;
}

/// A specific attribute that can obey a motion curve, say rotation
pub trait Animatable {
    fn animate(&mut self, motion: &MotionCurve) -> Result<(), AnimationError>;
}

#[derive(Debug, Clone)]
pub struct LinearAnimator;

impl LinearAnimator {
    pub fn new() -> Self {
        Self
    }
}

impl MotionBender for LinearAnimator {
    fn animate_between(&self, start_value: f64, end_value: f64) -> MotionCurve {
        let start = (IntervalPosition::START, start_value);
        let end = (IntervalPosition::END, end_value);
        MotionCurve {
            start,
            motion: vec![Cubic {
                p1: start,
                p2: end,
                p3: end,
            }],
        }
    }
}

/// Implement to convert abstracted animation into an end user (developer) deliver format.
///
/// For example, this would be implemented for Lottie, AndroidVectorDrawable, and possibly Web.
pub trait ToDeliveryFormat
where
    Self: Sized,
{
    fn generate(
        glyph: &AnimatedGlyph,
        bender: &dyn MotionBender,
        duration: Duration,
    ) -> Result<Self, ToDeliveryError>;
}
