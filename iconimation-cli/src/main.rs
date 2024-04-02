use std::str::FromStr;
use std::{fs, path::Path};

use bodymovin::Bodymovin as Lottie;
use clap::Parser;
use iconimation::android::AnimatedVectorDrawable;
use iconimation::ir::{Animation, FromAnimation};
use iconimation::plan::parse_plan;
use skrifa::instance::Location;
use skrifa::raw::types::InvalidTag;
use skrifa::raw::FontRef;
use skrifa::{MetadataProvider, Tag};
use thiserror::Error;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    #[clap(required(true))]
    command: String,

    #[arg(short, long)]
    #[clap(required(true))]
    font: String,

    #[arg(short, long)]
    #[clap(default_value = "lottie.json")]
    lottie_output: String,

    #[arg(short, long)]
    #[clap(default_value = "avd.xml")]
    android_output: String,
}

#[derive(Debug, Error)]
pub enum LocationError {
    #[error("Position must be a csv of tag:value pairs, e.g. FILL:1,wght:100")]
    InvalidPosition,
    #[error("Invalid tag '{0}'")]
    InvalidTag(InvalidTag),
    #[error("Font does not support tag '{0}'")]
    NoSuchAxis(Tag),
    #[error("Unable to parse value of '{0}'")]
    InvalidValue(Tag),
    #[error("Value for '{0}' must be in [{1:.2}, {2:.2}]")]
    OutOfBounds(Tag, f32, f32),
}

/// Avoid orphan rule
trait LocationParser {
    fn parse_location(&self, s: Option<&str>) -> Result<Location, LocationError>;
}

impl LocationParser for FontRef<'_> {
    fn parse_location(&self, s: Option<&str>) -> Result<Location, LocationError> {
        let Some(s) = s else {
            return Ok(Location::default());
        };
        let mut axis_positions = Vec::new();
        for raw_pos in s.split(',') {
            let parts: Vec<_> = raw_pos.split(':').collect();
            if parts.len() != 2 {
                return Err(LocationError::InvalidPosition);
            }
            let tag = Tag::from_str(parts[0]).map_err(LocationError::InvalidTag)?;
            let value: f32 = parts[1]
                .parse()
                .map_err(|_| LocationError::InvalidValue(tag))?;
            axis_positions.push((tag, value));
        }
        Ok(self.axes().location(axis_positions))
    }
}

fn main() {
    let args = Args::parse();

    let font_file = Path::new(args.font.as_str());
    let font_bytes = fs::read(font_file).unwrap();
    let font = FontRef::new(&font_bytes).unwrap();

    let (plan, glyph_shape) = parse_plan(&font, &args.command).unwrap();
    let animation = Animation::of_icon(&plan, &glyph_shape).unwrap();

    let lottie = Lottie::from_animation(&animation).unwrap();
    fs::write(
        &args.lottie_output,
        serde_json::to_string_pretty(&lottie).unwrap(),
    )
    .unwrap();
    eprintln!("Wrote Lottie {}", args.lottie_output);

    let avd = AnimatedVectorDrawable::from_animation(&animation).unwrap();
    fs::write(&args.android_output, avd.to_avd_xml().unwrap()).unwrap();
    eprintln!("Wrote AnimatedVectorDrawable {}", args.android_output);
}
