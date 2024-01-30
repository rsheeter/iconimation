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

#[derive(Debug, Error)]
pub enum SpringFitError {
    #[error("Did not reach equilibrium by {0:.3}")]
    NoEquilibrium(f64),
    #[error("We hit equilibrium immediately; no curve to smooth")]
    ImmediateEquilibrium,
}
