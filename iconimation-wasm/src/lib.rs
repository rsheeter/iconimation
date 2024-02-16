//! Animate arbitrary icons based on text commands

use std::{str::FromStr, sync::OnceLock};

use iconimation::{
    animate::Animation, default_template, ligate::icon_name_to_gid, GlyphShape, Template, ToLottie,
};
use kurbo::{Point, Rect};
use regex::{Captures, Regex};

use js_sys::{ArrayBuffer, Uint8Array};
use skrifa::{
    raw::{FontRef, TableProvider},
    MetadataProvider, Tag,
};

use wasm_bindgen::prelude::*;

#[derive(Debug, PartialEq)]
struct NameAndVariation<'a> {
    icon_name: &'a str,
    vary_from: Option<&'a str>,
    vary_to: Option<&'a str>,
}

impl<'a> NameAndVariation<'a> {
    fn from_captures(
        captures: &Captures<'a>,
        name_idx: usize,
        vary_from_idx: usize,
        vary_to_idx: usize,
    ) -> Result<Self, String> {
        let icon_name = captures
            .get(name_idx)
            .ok_or_else(|| "Unable to parse icon name".to_string())?;
        let vary_from = captures.get(vary_from_idx).map(|m| m.as_str());
        let vary_to = captures.get(vary_to_idx).map(|m| m.as_str());
        Ok(NameAndVariation {
            icon_name: icon_name.as_str(),
            vary_from,
            vary_to,
        })
    }
}

#[derive(Debug, PartialEq)]
enum Command<'a> {
    None(NameAndVariation<'a>),
    RotateDegrees(NameAndVariation<'a>, f64),
    ScaleFromTo(NameAndVariation<'a>, f64, f64),
    PulseWhole(NameAndVariation<'a>),
    PulseParts(NameAndVariation<'a>),
    TwirlWhole(NameAndVariation<'a>),
    TwirlParts(NameAndVariation<'a>),
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
        const ANIMATE: &str = r"^Animate\s+(\w+)\s*:\s*";
        const SPRING: &str = r"\s+using\s+([\w-]+)";
        const VARIATION: &str = r"(?:\s+vary\s+(\S+)\s+to\s+(\S+))?";
        static ROTATE: OnceLock<Regex> = OnceLock::new();
        static SCALE: OnceLock<Regex> = OnceLock::new();
        static ONLY_NAME: OnceLock<Regex> = OnceLock::new();

        let rotate = ROTATE.get_or_init(|| {
            Regex::new(
                &(ANIMATE.to_string() + r"rotate\s+(\d+)\s+degrees" + SPRING + VARIATION + "$"),
            )
            .unwrap()
        });
        let scale = SCALE.get_or_init(|| {
            Regex::new(
                &(ANIMATE.to_string() + r"scale\s+(\d+)\s+to\s+(\d+)" + SPRING + VARIATION + "$"),
            )
            .unwrap()
        });
        let only_name = ONLY_NAME.get_or_init(|| {
            Regex::new(
                &(ANIMATE.to_string()
                    + r"(pulse|pulse-whole|twirl|twirl-whole|none)"
                    + VARIATION
                    + "$"),
            )
            .unwrap()
        });

        Ok(if let Some(captures) = rotate.captures_at(animation, 0) {
            let nv = NameAndVariation::from_captures(&captures, 1, 4, 5)?;
            let degrees = get_f64("degrees", &captures, 2)?;
            Command::RotateDegrees(nv, degrees)
        } else if let Some(captures) = scale.captures_at(animation, 0) {
            let nv = NameAndVariation::from_captures(&captures, 1, 5, 6)?;
            let from = get_f64("from", &captures, 2)?;
            let to = get_f64("to", &captures, 3)?;
            Command::ScaleFromTo(nv, from, to)
        } else if let Some(captures) = only_name.captures_at(animation, 0) {
            let nv = NameAndVariation::from_captures(&captures, 1, 3, 4)?;
            let command = captures
                .get(2)
                .ok_or_else(|| "Unable to parse command".to_string())?;
            match command.as_str() {
                "none" => Command::None(nv),
                "pulse" => Command::PulseParts(nv),
                "pulse-whole" => Command::PulseWhole(nv),
                "twirl" => Command::TwirlParts(nv),
                "twirl-whole" => Command::TwirlWhole(nv),
                _ => return Err("Unrecognized command".to_string()),
            }
        } else {
            return Err("Unable to parse input".to_string());
        })
    }

    fn icon_name(&self) -> &str {
        match self {
            Command::None(nv, ..)
            | Command::RotateDegrees(nv, ..)
            | Command::ScaleFromTo(nv, ..)
            | Command::PulseWhole(nv, ..)
            | Command::PulseParts(nv, ..)
            | Command::TwirlWhole(nv, ..)
            | Command::TwirlParts(nv, ..) => nv.icon_name,
        }
    }

    fn variation(&self) -> Result<(Vec<(Tag, f32)>, Vec<(Tag, f32)>), String> {
        let nv = match self {
            Command::None(nv, ..)
            | Command::RotateDegrees(nv, ..)
            | Command::ScaleFromTo(nv, ..)
            | Command::PulseWhole(nv, ..)
            | Command::PulseParts(nv, ..)
            | Command::TwirlWhole(nv, ..)
            | Command::TwirlParts(nv, ..) => nv,
        };
        let from = nv
            .vary_from
            .map(parse_location)
            .unwrap_or_else(|| Ok(vec![]))?;
        let to = nv
            .vary_to
            .map(parse_location)
            .unwrap_or_else(|| Ok(vec![]))?;
        Ok((from, to))
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

fn parse_location<'a>(raw: &'a str) -> Result<Vec<(Tag, f32)>, String> {
    raw.split(',')
        .map(|kv| {
            let parts = kv.split(':').collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err("Invalid part".to_string());
            }
            let tag = Tag::from_str(parts[0]).map_err(|e| format!("Bad tag: {e}"))?;
            let value =
                f32::from_str(parts[1]).map_err(|e| format!("Bad value for '{tag}': {e}"))?;
            Ok((tag, value))
        })
        .collect::<Result<_, _>>()
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

    let (from, to) = command.variation()?;
    let from = font.axes().location(from);
    let to = font.axes().location(to);

    let glyph_shape = GlyphShape::new(&font, gid, from, Some(to))
        .map_err(|e| format!("Unable to create GlyphShape for {gid}: {e}"))?;

    let animation = command.animator(&glyph_shape);
    lottie
        .replace_shape(&animation)
        .map_err(|e| format!("Unable to animate {gid}: {e}"))?;

    Ok(serde_json::to_string_pretty(&lottie).unwrap())
}

#[cfg(test)]
mod tests {
    use crate::{Command, NameAndVariation};

    fn name_only<'a>(icon_name: &'a str) -> NameAndVariation<'a> {
        NameAndVariation {
            icon_name,
            vary_from: None,
            vary_to: None,
        }
    }

    fn varied<'a>(
        icon_name: &'a str,
        vary_from: &'a str,
        vary_to: &'a str,
    ) -> NameAndVariation<'a> {
        NameAndVariation {
            icon_name,
            vary_from: Some(vary_from),
            vary_to: Some(vary_to),
        }
    }

    #[test]
    fn parse_rotate() {
        let cmd = Command::parse("Animate settings: rotate 360 degrees using expressive-spatial")
            .unwrap();
        assert_eq!(Command::RotateDegrees(name_only("settings"), 360.0), cmd);
    }

    #[test]
    fn parse_scale() {
        let cmd = Command::parse("Animate check_circle: scale 0 to 100 using expressive-spatial")
            .unwrap();
        assert_eq!(
            Command::ScaleFromTo(name_only("check_circle"), 0.0, 100.0),
            cmd
        );
    }

    #[test]
    fn parse_pulse() {
        let cmd = Command::parse("Animate close: pulse").unwrap();
        assert_eq!(Command::PulseParts(name_only("close")), cmd);
    }

    #[test]
    fn parse_rotate_with_variation() {
        let cmd = Command::parse("Animate settings: rotate 360 degrees using expressive-spatial vary blah:99 to blah:101")
            .unwrap();
        assert_eq!(
            Command::RotateDegrees(varied("settings", "blah:99", "blah:101"), 360.0),
            cmd
        );
    }

    #[test]
    fn parse_scale_with_variation() {
        let cmd = Command::parse("Animate check_circle: scale 0 to 100 using expressive-spatial vary wght:400,FILL:1 to wght:700,FILL:0")
            .unwrap();
        assert_eq!(
            Command::ScaleFromTo(
                varied("check_circle", "wght:400,FILL:1", "wght:700,FILL:0"),
                0.0,
                100.0
            ),
            cmd
        );
    }

    #[test]
    fn parse_pulse_with_variation() {
        let cmd = Command::parse("Animate close: pulse vary FILL:0 to FILL:1").unwrap();
        assert_eq!(
            Command::PulseParts(varied("close", "FILL:0", "FILL:1")),
            cmd
        );
    }
}
