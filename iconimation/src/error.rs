//! Error types
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unable to draw: {0}")]
    DrawError(skrifa::outline::DrawError),
    #[error("No shapes updated")]
    NoShapesUpdated,
    #[error("No keyframes updated")]
    NoTransformsUpdated,
}

#[derive(Debug, Error)]
pub enum SpringBuildError {
    #[error("Damping must be >= 0")]
    InvalidDamping,
}
