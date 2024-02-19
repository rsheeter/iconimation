//! Simple algorithmic animations
//!
//! Typically supports both a whole-icon and parts mode where parts animate offset slightly in time.

use std::fmt::Debug;

use crate::animated_glyph::AnimatedGlyph;
use crate::animator::{Animated, IntervalPosition};
use crate::error::AnimationError;

#[derive(Debug)]
pub enum Animation {
    None,
    PulseParts,
    PulseWhole,
    TwirlWhole,
    TwirlParts,
}

const PULSE_SEQUENCE: &[(IntervalPosition, f64)] = &[
    (IntervalPosition::START, 100.0),
    (IntervalPosition::new(0.5), 125.0),
    (IntervalPosition::END, 100.0),
];

const TWIRL_SEQUENCE: &[(IntervalPosition, f64)] = &[
    (IntervalPosition::START, 0.0),
    (IntervalPosition::END, 360.0),
];

impl Animation {
    pub fn animate(&self, glyph: &mut AnimatedGlyph) -> Result<(), AnimationError> {
        match self {
            Animation::None => (),
            Animation::PulseWhole => {
                glyph.uniform_scale = Some(Animated::new(PULSE_SEQUENCE.to_owned())?)
            }
            Animation::TwirlWhole => {
                glyph.uniform_scale = Some(Animated::new(TWIRL_SEQUENCE.to_owned())?)
            }
            Animation::PulseParts => {
                glyph.group_for_piecewise_animation();
                glyph.uniform_scale = Some(Animated::new(PULSE_SEQUENCE.to_owned())?);
            }
            Animation::TwirlParts => {
                glyph.group_for_piecewise_animation();
                glyph.uniform_scale = Some(Animated::new(TWIRL_SEQUENCE.to_owned())?);
            }
        }
        Ok(())
    }
}
