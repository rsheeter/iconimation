//! Animate arbitrary icons based on text commands

use std::sync::OnceLock;

use iconimation::{
    animate::Animation, default_template, ligate::icon_name_to_gid, GlyphShape, Template, ToLottie,
};
use kurbo::{Point, Rect};
use regex::{Captures, Regex};

use js_sys::{ArrayBuffer, Uint8Array};
use skrifa::raw::{FontRef, TableProvider};

use wasm_bindgen::prelude::*;

#[derive(Debug, PartialEq)]
enum Command<'a> {
    RotateDegrees(&'a str, f64),
    ScaleFromTo(&'a str, f64, f64),
    PulseWhole(&'a str),
    PulseParts(&'a str),
    TwirlWhole(&'a str),
    TwirlParts(&'a str),
}

fn get_f64(name: &str, captures: &Captures<'_>, i: usize) -> Result<f64, String> {
    let raw = captures
        .get(i)
        .ok_or_else(|| format!("No match [{i}] for {name}"))?;
    raw.as_str()
        .parse()
        .map_err(|e| format!("{name} doesn't parse to f64: {e}, value '{}'", raw.as_str()))
}

impl Command<'_> {
    fn parse(animation: &str) -> Result<Command, String> {
        static ROTATE: OnceLock<Regex> = OnceLock::new();
        static SCALE: OnceLock<Regex> = OnceLock::new();
        static ONLY_NAME: OnceLock<Regex> = OnceLock::new();

        let rotate = ROTATE.get_or_init(|| {
            Regex::new(r"^Animate\s+(\w+)\s*:\s*rotate\s+(\d+)\s+degrees\s+using\s+([\w-]+)$")
                .unwrap()
        });
        let scale = SCALE.get_or_init(|| {
            Regex::new(r"^Animate\s+(\w+)\s*:\s*scale\s+(\d+)\s+to\s+(\d+)\s+using\s+([\w-]+)$")
                .unwrap()
        });
        let only_name = ONLY_NAME.get_or_init(|| {
            Regex::new(r"^Animate\s+(\w+)\s*:\s*(pulse|pulse-whole|twirl|twirl-whole)$").unwrap()
        });

        Ok(if let Some(captures) = rotate.captures_at(animation, 0) {
            let icon_name = captures
                .get(1)
                .ok_or_else(|| "Unable to parse icon name".to_string())?;
            let degrees = get_f64("degrees", &captures, 2)?;
            Command::RotateDegrees(icon_name.as_str(), degrees)
        } else if let Some(captures) = scale.captures_at(animation, 0) {
            let icon_name = captures
                .get(1)
                .ok_or_else(|| "Unable to parse icon name".to_string())?;
            let from = get_f64("from", &captures, 2)?;
            let to = get_f64("to", &captures, 3)?;
            Command::ScaleFromTo(icon_name.as_str(), from, to)
        } else if let Some(captures) = only_name.captures_at(animation, 0) {
            let icon_name = captures
                .get(1)
                .ok_or_else(|| "Unable to parse icon name".to_string())?;
            let command = captures
                .get(2)
                .ok_or_else(|| "Unable to parse command".to_string())?;
            match command.as_str() {
                "pulse" => Command::PulseParts(icon_name.as_str()),
                "pulse-whole" => Command::PulseWhole(icon_name.as_str()),
                "twirl" => Command::TwirlParts(icon_name.as_str()),
                "twirl-whole" => Command::TwirlWhole(icon_name.as_str()),
                _ => return Err("Unrecognized command".to_string()),
            }
        } else {
            return Err("Unable to parse input".to_string());
        })
    }

    fn icon_name(&self) -> &str {
        match self {
            Command::RotateDegrees(name, ..)
            | Command::ScaleFromTo(name, ..)
            | Command::PulseWhole(name, ..)
            | Command::PulseParts(name, ..)
            | Command::TwirlWhole(name, ..)
            | Command::TwirlParts(name, ..) => name,
        }
    }

    fn animator<'a>(&self, to_lottie: &'a dyn ToLottie) -> Animation<'a> {
        match self {
            Command::PulseParts(..) => Animation::PulseParts(to_lottie),
            Command::PulseWhole(..) => Animation::PulseWhole(to_lottie),
            Command::TwirlParts(..) => Animation::TwirlParts(to_lottie),
            Command::TwirlWhole(..) => Animation::TwirlWhole(to_lottie),
            _ => Animation::None(to_lottie),
        }
    }
}

#[wasm_bindgen]
pub fn generate_lottie(raw_font: &ArrayBuffer, animation: String) -> Result<String, String> {
    let command = Command::parse(&animation)?;

    let rust_buf = Uint8Array::new(raw_font).to_vec();
    let font = FontRef::new(&rust_buf).map_err(|e| format!("FontRef::new failed: {e}"))?;
    let upem = font.head().unwrap().units_per_em() as f64;
    let font_drawbox: Rect = (Point::ZERO, Point::new(upem, upem)).into();

    let gid = icon_name_to_gid(&font, command.icon_name())
        .map_err(|e| format!("Unable to determine icon gid {e}"))?;

    let mut lottie = default_template(&font_drawbox);

    let glyph_shape = GlyphShape::new(&font, gid)
        .map_err(|e| format!("Unable to create GlyphShape for {gid}: {e}"))?;

    let animation = command.animator(&glyph_shape);
    lottie
        .replace_shape(&animation)
        .map_err(|e| format!("Unable to animate {gid}: {e}"))?;

    Ok(serde_json::to_string_pretty(&lottie).unwrap())
}

#[cfg(test)]
mod tests {
    use crate::Command;

    #[test]
    fn parse_rotate() {
        let cmd = Command::parse("Animate settings: rotate 360 degrees using expressive-spatial")
            .unwrap();
        assert_eq!(Command::RotateDegrees("settings", 360.0), cmd);
    }

    #[test]
    fn parse_scale() {
        let cmd = Command::parse("Animate check_circle: scale 0 to 100 using expressive-spatial")
            .unwrap();
        assert_eq!(Command::ScaleFromTo("check_circle", 0.0, 100.0), cmd);
    }

    #[test]
    fn parse_pulse() {
        let cmd = Command::parse("Animate close: pulse").unwrap();
        assert_eq!(Command::PulseParts("close"), cmd);
    }
}
