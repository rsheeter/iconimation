use std::str::FromStr;
use std::{fs, path::Path};

use clap::Parser;
use iconimation::command::parse_command;
use iconimation::generate_lottie;
use skrifa::instance::Location;
use skrifa::raw::types::InvalidTag;
use skrifa::raw::FontRef;
use skrifa::{MetadataProvider, Tag};
use thiserror::Error;

#[derive(Parser)]
struct Args {
    /// Whether to emit additional debug info
    #[arg(long)]
    debug: bool,

    #[arg(short, long)]
    #[clap(required(true))]
    command: String,

    #[arg(short, long)]
    #[clap(required(true))]
    font: String,

    #[arg(short, long)]
    #[clap(default_value = "output.json")]
    out_file: String,
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

    let (command, glyph_shape) = parse_command(&font, &args.command).unwrap();

    let font_drawbox = glyph_shape.drawbox();
    eprintln!("font_drawbox {:?}", font_drawbox);

    let lottie = generate_lottie(&font, &command, &glyph_shape).unwrap();

    fs::write(
        &args.out_file,
        serde_json::to_string_pretty(&lottie).unwrap(),
    )
    .unwrap();
    eprintln!("Wrote {}", args.out_file);
}
