//! Animate arbitrary icons based on text commands

use bodymovin::Bodymovin as Lottie;
use iconimation::{
    android::AnimatedVectorDrawable,
    ir::{Animation, FromAnimation},
    plan::parse_plan,
};

use js_sys::{ArrayBuffer, Uint8Array};
use serde::Serialize;
use skrifa::raw::FontRef;

use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Animations {
    lottie: String,
    avd: String,
    debug: String,
}

#[wasm_bindgen]
pub fn generate_animation(raw_font: &ArrayBuffer, raw_command: String) -> Result<String, String> {
    let rust_buf = Uint8Array::new(raw_font).to_vec();
    let font = FontRef::new(&rust_buf).map_err(|e| format!("FontRef::new failed: {e}"))?;

    let (plan, glyph_shape) = parse_plan(&font, &raw_command).map_err(|e| format!("{e}"))?;
    let animation = Animation::of_icon(&plan, &glyph_shape)
        .map_err(|e| format!("Animation::new failed: {e}"))?;

    let lottie =
        Lottie::from_animation(&animation).map_err(|e| format!("Lottie generation failed: {e}"))?;
    let avd = AnimatedVectorDrawable::from_animation(&animation)
        .map_err(|e| format!("AVD generation failed: {e}"))?;

    Ok(serde_json::to_string_pretty(&Animations {
        lottie: serde_json::to_string_pretty(&lottie)
            .map_err(|e| format!("Lottie to json failed: {e}"))?,
        avd: avd
            .to_avd_xml()
            .map_err(|e| format!("AVD to xml failed: {e}"))?,
        debug: "".to_string(),
    })
    .unwrap())
}
