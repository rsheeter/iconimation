//! Quick & dirty text input to icon animation definition

use std::{str::FromStr, sync::OnceLock};

use regex::{Captures, Regex};
use skrifa::{raw::FontRef, MetadataProvider, Tag};

use crate::{error::Error, ligate::icon_name_to_gid, spring::Spring, GlyphShape};

#[derive(Debug, PartialEq)]
pub struct NameAndVariation<'a> {
    icon_name: &'a str,
    spring: Option<Spring>,
    vary_from: Option<&'a str>,
    vary_to: Option<&'a str>,
}

impl<'a> NameAndVariation<'a> {
    fn from_captures(
        captures: &Captures<'a>,
        name_idx: usize,
        spring_idx: usize,
        vary_from_idx: usize,
        vary_to_idx: usize,
    ) -> Result<Self, Error> {
        let icon_name = captures
            .get(name_idx)
            .ok_or(Error::NoCapture("icon name", name_idx))?
            .as_str();
        let spring = captures
            .get(spring_idx)
            .map(|m| Spring::from_str(m.as_str()))
            .transpose()
            .map_err(|_| Error::UnrecognizedSpring)?;
        let vary_from = captures.get(vary_from_idx).map(|m| m.as_str());
        let vary_to = captures.get(vary_to_idx).map(|m| m.as_str());
        Ok(NameAndVariation {
            icon_name,
            spring,
            vary_from,
            vary_to,
        })
    }
}

type UserLocation = Vec<(Tag, f32)>;

#[derive(Debug, PartialEq)]
pub enum Command<'a> {
    None(NameAndVariation<'a>),
    RotateDegrees(NameAndVariation<'a>, f64),
    ScaleFromTo(NameAndVariation<'a>, f64, f64),
    PulseWhole(NameAndVariation<'a>),
    PulseParts(NameAndVariation<'a>),
    TwirlWhole(NameAndVariation<'a>),
    TwirlParts(NameAndVariation<'a>),
}

fn get_f64(name: &'static str, captures: &Captures<'_>, i: usize) -> Result<f64, Error> {
    let raw = captures.get(i).ok_or(Error::NoCapture(name, i))?;
    raw.as_str().parse::<f64>().map_err(Error::InvalidF64)
}

impl Command<'_> {
    fn parse(animation: &str) -> Result<Command, Error> {
        const ANIMATE: &str = r"^Animate\s+(\w+)\s*:\s*";
        const SPRING: &str = r"(?:\s+using\s+([\w-]+))?";
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
                    + r"(pulse|pulse-whole|twirl|twirl-whole)?"
                    + SPRING
                    + VARIATION
                    + "$"),
            )
            .unwrap()
        });

        Ok(if let Some(captures) = rotate.captures_at(animation, 0) {
            let nv = NameAndVariation::from_captures(&captures, 1, 3, 4, 5)?;
            let degrees = get_f64("degrees", &captures, 2)?;
            Command::RotateDegrees(nv, degrees)
        } else if let Some(captures) = scale.captures_at(animation, 0) {
            let nv = NameAndVariation::from_captures(&captures, 1, 4, 5, 6)?;
            let from = get_f64("from", &captures, 2)?;
            let to = get_f64("to", &captures, 3)?;
            Command::ScaleFromTo(nv, from, to)
        } else if let Some(captures) = only_name.captures_at(animation, 0) {
            eprintln!("only_name captures\n{captures:?}");
            let nv = NameAndVariation::from_captures(&captures, 1, 3, 4, 5)?;
            let command = captures.get(2).map(|m| m.as_str()).unwrap_or("none");
            match command {
                "none" => Command::None(nv),
                "pulse" => Command::PulseParts(nv),
                "pulse-whole" => Command::PulseWhole(nv),
                "twirl" => Command::TwirlParts(nv),
                "twirl-whole" => Command::TwirlWhole(nv),
                _ => return Err(Error::UnrecognizedCommand),
            }
        } else {
            return Err(Error::UnrecognizedCommand);
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

    pub fn spring(&self) -> Option<Spring> {
        match self {
            Command::None(nv, ..)
            | Command::RotateDegrees(nv, ..)
            | Command::ScaleFromTo(nv, ..)
            | Command::PulseWhole(nv, ..)
            | Command::PulseParts(nv, ..)
            | Command::TwirlWhole(nv, ..)
            | Command::TwirlParts(nv, ..) => nv.spring,
        }
    }

    pub fn variation(&self) -> Result<(UserLocation, UserLocation), Error> {
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
}

fn parse_location(raw: &str) -> Result<UserLocation, Error> {
    raw.split(',')
        .map(|kv| {
            let parts = kv.split(':').collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(Error::InvalidLocation);
            }
            let tag = Tag::from_str(parts[0]).map_err(Error::InvalidTag)?;
            let value = f32::from_str(parts[1]).map_err(Error::InvalidF64)?;
            Ok((tag, value))
        })
        .collect::<Result<_, _>>()
}

pub fn parse_command<'a, 'b>(
    font: &'a FontRef,
    command: &'b str,
) -> Result<(Command<'b>, GlyphShape<'a>), Error> {
    let command = Command::parse(command)?;

    let gid = icon_name_to_gid(font, command.icon_name()).map_err(Error::IconNameError)?;

    let (raw_from, raw_to) = command.variation()?;
    let from = font.axes().location(raw_from);
    let to = font.axes().location(raw_to);

    let glyph_shape = GlyphShape::new(font, gid, from, Some(to))?;

    Ok((command, glyph_shape))
}

#[cfg(test)]
mod tests {
    use crate::spring::Spring;

    use super::{Command, NameAndVariation};

    impl<'a> From<&'a str> for NameAndVariation<'a> {
        fn from(icon_name: &'a str) -> Self {
            NameAndVariation {
                icon_name,
                spring: None,
                vary_from: None,
                vary_to: None,
            }
        }
    }

    impl<'a> From<(&'a str, Spring)> for NameAndVariation<'a> {
        fn from(value: (&'a str, Spring)) -> Self {
            NameAndVariation {
                icon_name: value.0,
                spring: Some(value.1),
                vary_from: None,
                vary_to: None,
            }
        }
    }

    impl<'a> From<(&'a str, &'a str, &'a str)> for NameAndVariation<'a> {
        fn from(value: (&'a str, &'a str, &'a str)) -> Self {
            NameAndVariation {
                icon_name: value.0,
                spring: None,
                vary_from: Some(value.1),
                vary_to: Some(value.2),
            }
        }
    }

    impl<'a> From<(&'a str, Spring, &'a str, &'a str)> for NameAndVariation<'a> {
        fn from(value: (&'a str, Spring, &'a str, &'a str)) -> Self {
            NameAndVariation {
                icon_name: value.0,
                spring: Some(value.1),
                vary_from: Some(value.2),
                vary_to: Some(value.3),
            }
        }
    }

    #[test]
    fn parse_rotate_with_spring() {
        let cmd = Command::parse("Animate settings: rotate 360 degrees using expressive-spatial")
            .unwrap();
        assert_eq!(
            Command::RotateDegrees(("settings", Spring::expressive_spatial()).into(), 360.0),
            cmd
        );
    }

    #[test]
    fn parse_scale() {
        let cmd = Command::parse("Animate check_circle: scale 0 to 100").unwrap();
        assert_eq!(
            Command::ScaleFromTo(("check_circle").into(), 0.0, 100.0),
            cmd
        );
    }

    #[test]
    fn parse_pulse() {
        let cmd = Command::parse("Animate close: pulse").unwrap();
        assert_eq!(Command::PulseParts(("close").into()), cmd);
    }

    #[test]
    fn parse_rotate_with_variation() {
        let cmd = Command::parse(
            "Animate settings: rotate 360 degrees using smooth-spatial vary blah:99 to blah:101",
        )
        .unwrap();
        assert_eq!(
            Command::RotateDegrees(
                ("settings", Spring::smooth_spatial(), "blah:99", "blah:101").into(),
                360.0
            ),
            cmd
        );
    }

    #[test]
    fn parse_minimal_twirl() {
        let cmd = Command::parse("Animate an_icon: twirl-whole").unwrap();
        assert_eq!(Command::TwirlWhole(("an_icon").into()), cmd);
    }

    #[test]
    fn parse_only_variation() {
        let cmd = Command::parse("Animate an_icon: vary FILL:0 to FILL:1").unwrap();
        assert_eq!(Command::None(("an_icon", "FILL:0", "FILL:1").into()), cmd);
    }

    #[test]
    fn parse_scale_with_variation_and_spring() {
        let cmd = Command::parse("Animate check_circle: scale 0 to 100 using expressive-spatial vary wght:400,FILL:1 to wght:700,FILL:0")
            .unwrap();
        assert_eq!(
            Command::ScaleFromTo(
                (
                    "check_circle",
                    Spring::expressive_spatial(),
                    "wght:400,FILL:1",
                    "wght:700,FILL:0"
                )
                    .into(),
                0.0,
                100.0
            ),
            cmd
        );
    }

    #[test]
    fn parse_pulse_with_variation_and_spring() {
        let cmd =
            Command::parse("Animate close: pulse using standard vary FILL:0 to FILL:1").unwrap();
        assert_eq!(
            Command::PulseParts(("close", Spring::standard(), "FILL:0", "FILL:1").into()),
            cmd
        );
    }
}
