use std::{fs, path::Path};

use bodymovin::Bodymovin as Lottie;
use clap::Parser;
use clap::ValueEnum;
use iconimation::animate::Animation;
use iconimation::debug_pen::DebugPen;
use iconimation::default_template;
use iconimation::Template;
use kurbo::Point;
use kurbo::Rect;
use skrifa::raw::FontRef;
use skrifa::raw::TableProvider;
use skrifa::MetadataProvider;

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
    fn to_lib(&self) -> Animation {
        match self {
            CliAnimation::None => Animation::None,
            CliAnimation::PulseWhole => Animation::PulseWhole,
            CliAnimation::PulseParts => Animation::PulseParts,
            CliAnimation::TwirlWhole => Animation::TwirlWhole,
            CliAnimation::TwirlParts => Animation::TwirlParts,
        }
    }
}

#[derive(Parser)]
struct Args {
    /// Whether to emit additional debug info
    #[arg(long)]
    debug: bool,

    #[clap(value_enum, required(true))]
    #[arg(long)]
    animation: CliAnimation,

    #[arg(long)]
    codepoint: String,

    #[arg(long)]
    template: Option<String>,

    #[arg(long)]
    #[clap(required(true))]
    font: String,

    #[arg(long)]
    #[clap(default_value = "output.json")]
    out_file: String,
}

fn main() {
    let args = Args::parse();

    assert!(
        args.codepoint.starts_with("0x"),
        "Codepoint must start with 0x"
    );
    let codepoint = u32::from_str_radix(&args.codepoint[2..], 16).unwrap();

    let font_file = Path::new(args.font.as_str());
    let font_bytes = fs::read(font_file).unwrap();
    let font = FontRef::new(&font_bytes).unwrap();
    let upem = font.head().unwrap().units_per_em() as f64;
    let font_drawbox: Rect = (Point::ZERO, Point::new(upem, upem)).into();
    let outline_loader = font.outline_glyphs();

    let gid = font
        .charmap()
        .map(codepoint)
        .unwrap_or_else(|| panic!("No gid for 0x{codepoint:04x}"));
    let glyph = outline_loader
        .get(gid)
        .unwrap_or_else(|| panic!("No outline for 0x{codepoint:04x} (gid {gid})"));

    if args.debug {
        let mut pen = DebugPen::new(Rect::new(0.0, 0.0, upem, upem));
        glyph
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

    let animation = args.animation.to_lib();
    lottie
        .replace_shape(&font_drawbox, &glyph, animation.animator().as_ref())
        .expect("Failed to replace shape");

    fs::write(
        &args.out_file,
        serde_json::to_string_pretty(&lottie).unwrap(),
    )
    .unwrap();
    eprintln!("Wrote {}", args.out_file);
}
