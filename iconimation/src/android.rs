//! Produce an output suitable for Android, e.g. an AnimatedVectorDrawable, from an Animation

use serde::Serialize;

use crate::{error::AndroidError, ir::FromAnimation};

/// An in memory representation of an [AndroidVectorDrawable](https://developer.android.com/reference/android/graphics/drawable/AnimatedVectorDrawable)
///
/// Limited to capabilities needed for icon animation. Can emit a [[single-file](https://developer.android.com/reference/android/graphics/drawable/AnimatedVectorDrawable#define-an-animatedvectordrawable-all-in-one-xml-file)
/// representation for use in Android projects.
#[derive(Serialize)]
pub struct AnimatedVectorDrawable {}

impl FromAnimation for AnimatedVectorDrawable {
    type Err = AndroidError;

    fn from_animation(_animation: &crate::ir::Animation) -> Result<Self, Self::Err> {
        eprintln!("TODO: make an AVD");
        Ok(AnimatedVectorDrawable {})
    }
}
