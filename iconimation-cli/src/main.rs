use std::str::FromStr;
use std::{fs, path::Path};

use bodymovin::Bodymovin as Lottie;
use clap::Parser;
use clap::ValueEnum;
use iconimation::animate::Animation;
use iconimation::debug_pen::DebugPen;
use iconimation::default_template;
use iconimation::ligate::icon_name_to_gid;
use iconimation::spring::Spring;
use iconimation::GlyphShape;
use iconimation::Template;
use skrifa::instance::Location;
use skrifa::raw::types::InvalidTag;
use skrifa::raw::FontRef;
use skrifa::{MetadataProvider, Tag};
use thiserror::Error;

/// Clap-friendly version of [Animation]
#[derive(ValueEnum, Clone, Debug)]
pub enum CliAnimation {
    None,
    PulseWhole,
    PulseParts,
    TwirlWhole,
    TwirlParts,
}

impl CliAnimation {
    fn to_lib<'a>(&self, shape: &'a GlyphShape) -> Animation<'a> {
        match self {
            CliAnimation::None => Animation::None(shape),
            CliAnimation::PulseWhole => Animation::PulseWhole(shape),
            CliAnimation::PulseParts => Animation::PulseParts(shape),
            CliAnimation::TwirlWhole => Animation::TwirlWhole(shape),
            CliAnimation::TwirlParts => Animation::TwirlParts(shape),
        }
    }
}

#[derive(Parser)]
struct Args {
    /// Whether to emit additional debug info
    #[arg(long)]
    debug: bool,

    /// Whether to generate spring-based animation between keyframes
    #[arg(long)]
    spring: bool,

    #[clap(value_enum, required(true))]
    #[arg(long)]
    animation: CliAnimation,

    #[arg(long)]
    icon: String,

    /// CSV of axis positions in user coords. If unset, the default location. E.g. FILL:0,wght:100
    #[arg(long)]
    from: Option<String>,

    /// CSV of axis positions in user coords. If unset, the default location. E.g. FILL:1,wght:700
    #[arg(long)]
    to: Option<String>,

    #[arg(long)]
    template: Option<String>,

    #[arg(long)]
    #[clap(required(true))]
    font: String,

    #[arg(long)]
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

    let gid = if args.icon.starts_with("0x") {
        let codepoint = u32::from_str_radix(&args.icon[2..], 16).unwrap();
        font.charmap()
            .map(codepoint)
            .unwrap_or_else(|| panic!("No gid for 0x{codepoint:04x}"))
    } else {
        icon_name_to_gid(&font, &args.icon)
            .unwrap_or_else(|e| panic!("Unable to resolve '{}' to a glyph id: {e}", args.icon))
    };

    let start = font
        .parse_location(args.from.as_deref())
        .unwrap_or_else(|e| panic!("Unable to parse --from: {e}"));
    let end = font
        .parse_location(args.to.as_deref())
        .unwrap_or_else(|e| panic!("Unable to parse --to: {e}"));

    let glyph_shape =
        GlyphShape::new(&font, gid, start, Some(end)).expect("Unable to create replacement");
    let font_drawbox = glyph_shape.drawbox();
    eprintln!("font_drawbox {:?}", font_drawbox);

    if args.debug {
        let mut pen = DebugPen::new(font_drawbox);
        font.outline_glyphs()
            .get(gid)
            .unwrap_or_else(|| panic!("No glyph for {gid}"))
            .draw(skrifa::instance::Size::unscaled(), &mut pen)
            .unwrap();
        let debug_out = Path::new(&args.out_file).with_extension("svg");
        fs::write(debug_out, pen.to_svg()).unwrap();
        eprintln!("Wrote debug svg {}", args.out_file);
    }

    let mut lottie = if let Some(template) = args.template {
        Lottie::load(template).expect("Unable to load custom template")
    } else {
        default_template(&font_drawbox)
    };

    let animation = args.animation.to_lib(&glyph_shape);
    lottie.replace_shape(&animation).expect("Failed to animate");

    if args.spring {
        lottie
            .spring(Spring::expressive_spatial())
            .expect("Failed to apply spring-based animation");
    }

    fs::write(
        &args.out_file,
        serde_json::to_string_pretty(&lottie).unwrap(),
    )
    .unwrap();
    eprintln!("Wrote {}", args.out_file);
}
