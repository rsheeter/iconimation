//! Animate arbitrary icons based on text commands

use bodymovin::Bodymovin as Lottie;
use iconimation::{
    command::{parse_command, Command},
    lottie_template, GlyphShape, Template,
};

use js_sys::{ArrayBuffer, Uint8Array};
use kurbo::{Point, Rect};
use serde::Serialize;
use skrifa::{
    raw::{FontRef, TableProvider},
    MetadataProvider,
};

use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Animations {
    lottie: Lottie,
    avd: String,
    debug: String,
}

fn generate_lottie(
    font: &FontRef,
    command: &Command,
    glyph_shape: &GlyphShape,
) -> Result<Lottie, String> {
    let upem = font.head().unwrap().units_per_em() as f64;
    let font_drawbox: Rect = (Point::ZERO, Point::new(upem, upem)).into();

    let mut lottie = lottie_template(&font_drawbox);
    lottie
        .replace_shape(glyph_shape)
        .map_err(|e| format!("replace_shape failed: {e}"))?;
    if let Some(spring) = command.spring() {
        lottie
            .spring(spring)
            .map_err(|e| format!("Spring failed: {e}"))?;
    }
    Ok(lottie)
}

fn generate_animated_vector_drawable(
    _font: &FontRef,
    _command: &Command,
    _glyph_shape: &GlyphShape,
) -> Result<String, String> {
    Ok("TODO: avd".to_string())
    // let upem = font.head().unwrap().units_per_em() as f64;
    // let font_drawbox: Rect = (Point::ZERO, Point::new(upem, upem)).into();

    // let mut lottie = lottie_template(&font_drawbox);
    // lottie
    //     .replace_shape(glyph_shape)
    //     .map_err(|e| format!("replace_shape failed: {e}"))?;
    // if let Some(spring) = command.spring() {
    //     lottie
    //         .spring(spring)
    //         .map_err(|e| format!("Spring failed: {e}"))?;
    // }
    // Ok(lottie)
}

#[wasm_bindgen]
pub fn generate_animation(raw_font: &ArrayBuffer, raw_command: String) -> Result<String, String> {
    let rust_buf = Uint8Array::new(raw_font).to_vec();
    let font = FontRef::new(&rust_buf).map_err(|e| format!("FontRef::new failed: {e}"))?;

    let (command, glyph_shape) = parse_command(&font, &raw_command).map_err(|e| format!("{e}"))?;

    let (raw_from, raw_to) = command
        .variation()
        .map_err(|e| format!("varation() failed: {e}"))?;
    let from = font.axes().location(&raw_from);
    let to = font.axes().location(&raw_to);

    let debug = format!(
        "{command:?}\n{raw_from:?} location {from:?}\n{raw_to:?} location {to:?}\naxes {}",
        font.axes()
            .iter()
            .map(|a| a.tag().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let lottie = generate_lottie(&font, &command, &glyph_shape)?;
    let avd = generate_animated_vector_drawable(&font, &command, &glyph_shape)?;

    Ok(serde_json::to_string_pretty(&Animations { lottie, avd, debug }).unwrap())
}
