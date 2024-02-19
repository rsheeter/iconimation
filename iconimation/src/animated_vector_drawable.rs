//! Converts [`AnimatedGlyph`] to [`AndroidVectorDrawable`]`

#[derive(Serialize)]
pub struct AndroidVectorDrawable;

use std::time::Duration;

use serde::Serialize;

use crate::{
    animated_glyph::AnimatedGlyph,
    animator::{MotionBender, ToDeliveryFormat},
};

impl ToDeliveryFormat for AndroidVectorDrawable {
    fn generate(
        glyph: &AnimatedGlyph,
        bender: &dyn MotionBender,
        duration: Duration,
    ) -> Result<Self, crate::error::ToDeliveryError> {
        eprintln!("TODO: AVD");
        Ok(AndroidVectorDrawable)
    }
}
