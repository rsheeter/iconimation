use std::{fs, path::Path};

use bodymovin::Bodymovin as Lottie;
use clap::Parser;
use clap::ValueEnum;
use iconimation::animate::Animation;
use iconimation::debug_pen::DebugPen;
use iconimation::default_template;
use iconimation::ligate::icon_name_to_gid;
use iconimation::AndroidSpring;
use iconimation::Spring;
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

    /// Whether to generate spring-based animation between keyframes
    #[arg(long)]
    spring: bool,

    #[clap(value_enum, required(true))]
    #[arg(long)]
    animation: CliAnimation,

    #[arg(long)]
    icon: String,

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

    let font_file = Path::new(args.font.as_str());
    let font_bytes = fs::read(font_file).unwrap();
    let font = FontRef::new(&font_bytes).unwrap();
    let upem = font.head().unwrap().units_per_em() as f64;
    let font_drawbox: Rect = (Point::ZERO, Point::new(upem, upem)).into();
    eprintln!("font_drawbox {font_drawbox:?}");
    let outline_loader = font.outline_glyphs();

    let gid = if args.icon.starts_with("0x") {
        let codepoint = u32::from_str_radix(&args.icon[2..], 16).unwrap();
        font.charmap()
            .map(codepoint)
            .unwrap_or_else(|| panic!("No gid for 0x{codepoint:04x}"))
    } else {
        icon_name_to_gid(&font, &args.icon)
            .unwrap_or_else(|e| panic!("Unable to resolve '{}' to a glyph id: {e}", args.icon))
    };

    let glyph = outline_loader
        .get(gid)
        .unwrap_or_else(|| panic!("No outline for {} (gid {gid})", args.icon));

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

    let spring: Spring = AndroidSpring {
        damping: 0.8,
        stiffness: 380.0,
        ..Default::default()
    }
    .into();

    if args.spring {
        lottie
            .spring(spring)
            .expect("Failed to apply spring-based animation");
    }

    fs::write(
        &args.out_file,
        serde_json::to_string_pretty(&lottie).unwrap(),
    )
    .unwrap();
    eprintln!("Wrote {}", args.out_file);
}
