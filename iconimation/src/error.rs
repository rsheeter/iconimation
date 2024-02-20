//! Error types
use skrifa::GlyphId;
use thiserror::Error;

use crate::animator::IntervalPosition;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unable to draw: {0}")]
    DrawError(skrifa::outline::DrawError),
    #[error("No shapes updated")]
    NoShapesUpdated,
    #[error("No keyframes updated")]
    NoTransformsUpdated,
    #[error("No outline for {0}")]
    NoOutline(GlyphId),
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
    #[error("Unable to draw: {0}")]
    DrawError(skrifa::outline::DrawError),
    #[error("0 keyframes makes for a dull animated value")]
    NoKeyframes,
    #[error("Multiple keyframes at the same time")]
    DuplicateKeyframes,
    #[error("The glyph is not a simple group containing only paths")]
    NotAGroupOfPaths,
}

#[derive(Debug, Error)]
pub enum ToDeliveryError {
    #[error("Unable to convert to Lottie paths, {0}")]
    PathConversionError(Error),
    #[error("Incompatible path sequence at t={0:?}")]
    IncompatiblePath(IntervalPosition),
    #[error("Unexpected animation at t={0:?}")]
    UnexpectedAnimation(IntervalPosition),
}
