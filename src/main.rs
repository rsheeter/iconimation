use std::{fs, path::Path};

use clap::Parser;
use iconimation::animate;
use iconimation::animate::Animator;
use iconimation::default_template;
use iconimation::Template;
use kurbo::Point;
use kurbo::Rect;
use skrifa::{
    raw::{FontRef, TableProvider},
    MetadataProvider,
};

#[derive(Parser)]
struct Args {
    #[clap(value_enum, required(true))]
    #[arg(long)]
    animation: Animation,

    #[arg(long)]
    codepoint: String,

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

    let mut lottie = default_template(&font_drawbox);
    lottie
        .replace_shape(&font_drawbox, &glyph, args.animation.animator())
        .unwrap();

    fs::write(
        &args.out_file,
        serde_json::to_string_pretty(&lottie).unwrap(),
    )
    .unwrap();
    eprintln!("Wrote {}", args.out_file);
}
