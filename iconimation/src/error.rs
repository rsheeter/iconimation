//! Error types
use std::num::ParseFloatError;

use kurbo::BezPath;
use skrifa::{outline::DrawError, raw::ReadError, GlyphId};
use thiserror::Error;
use write_fonts::types::InvalidTag;

use crate::{ir::Keyframed, spring::AnimatedValueType};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unable to draw: {0}")]
    DrawError(skrifa::outline::DrawError),
    #[error("No shapes updated")]
    NoShapesUpdated,
    #[error("No keyframes updated")]
    NoTransformsUpdated,
    #[error("No placeholders found")]
    NoPlaceholders,
    #[error("No outline for {0}")]
    NoOutline(GlyphId),
    #[error("Inconsistent number of {0:?} values: {1:?} vs {2:?}")]
    ValueLengthMismatch(AnimatedValueType, Vec<f64>, Vec<f64>),
    #[error("{0}")]
    IconNameError(IconNameError),
    #[error("Invalid variation parameters")]
    InvalidLocation,
    #[error("Invalid tag")]
    InvalidTag(InvalidTag),
    #[error("Invalid f64 {0}")]
    InvalidF64(#[from] ParseFloatError),
    #[error("No capture for {0} at {1}")]
    NoCapture(&'static str, usize),
    #[error("Unrecognized command")]
    UnrecognizedCommand,
    #[error("Unrecognized spring")]
    UnrecognizedSpring,
}

#[derive(Debug, Error)]
pub enum SpringBuildError {
    #[error("Damping must be >= 0")]
    InvalidDamping,
}

#[derive(Debug, Error)]
pub enum IconNameError {
    #[error("{0}")]
    ReadError(skrifa::raw::ReadError),
    #[error("No character mapping for '{0}'")]
    UnmappedCharError(char),
    #[error("The icon name '{0}' resolved to 0 glyph ids")]
    NoGlyphIds(String),
    #[error("The icon name '{0}' has no ligature")]
    NoLigature(String),
}

#[derive(Debug, Error)]
pub enum AnimationError {
    #[error("The 'head' table isn't present, {0}")]
    NoHeadTable(ReadError),
    #[error("Unable to draw {0:?}: {1}")]
    DrawError(GlyphId, DrawError),
    #[error("Must have at least 1 keyframe")]
    NoKeyframes,
    #[error("Keyframe frame must be unique, multiple definitions of {0}")]
    MultipleValuesForFrame(f64),
}

#[derive(Debug, Error)]
pub enum LottieError {
    #[error("Interpolation-incompatible paths: {0:?}")]
    IncompatiblePaths(Keyframed<BezPath>),
}

#[derive(Debug, Error)]
pub enum AndroidError {}

#[derive(Debug, Error)]
pub enum CubicApproximationError {
    #[error("Unrecognized spring")]
    UnrecognizedSpring,
    #[error("Input took too long to reach equilibrium")]
    RanTooLong,
}
