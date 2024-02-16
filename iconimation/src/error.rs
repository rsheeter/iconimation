//! Error types
use skrifa::GlyphId;
use thiserror::Error;

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
