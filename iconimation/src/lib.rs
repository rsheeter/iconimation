//! Shove glyphs from a variable font into a Lottie template.

pub mod android;
mod bezop;
pub mod error;
pub mod ir;
pub mod ligate;
pub mod lottie;
pub mod plan;
pub mod spring;
pub mod spring2cubic;

use std::fmt::Debug;

use kurbo::{BezPath, PathEl, Point, Rect};
use skrifa::{
    instance::Location,
    raw::{FontRef, TableProvider},
    GlyphId, MetadataProvider, OutlineGlyph,
};

use crate::error::Error;

pub struct GlyphShape<'a> {
    font: &'a FontRef<'a>,
    glyph: OutlineGlyph<'a>,
    gid: GlyphId,
    start: Location,
    // If set, animate from start => end
    end: Option<Location>,
}

impl<'a> Debug for GlyphShape<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphShape")
            .field("gid", &self.gid)
            .finish()
    }
}

impl<'a> GlyphShape<'a> {
    pub fn new(
        font: &'a FontRef<'a>,
        gid: GlyphId,
        start: Location,
        mut end: Option<Location>,
    ) -> Result<Self, Error> {
        let outline_loader = font.outline_glyphs();
        let Some(glyph) = outline_loader.get(gid) else {
            return Err(Error::NoOutline(gid));
        };
        if let Some(end_loc) = &end {
            if start.coords() == end_loc.coords() {
                end = None;
            }
        }
        Ok(Self {
            font,
            glyph,
            gid,
            start,
            end,
        })
    }

    pub fn drawbox(&self) -> Rect {
        let upem = self.font.head().unwrap().units_per_em() as f64;
        (Point::ZERO, Point::new(upem, upem)).into()
    }
}

/// Lists the path commands, e.g. MCLZ, used by the path.
///
/// Paths with the same commands are interpolation compatible.
fn path_commands(bez: &BezPath) -> String {
    bez.elements()
        .iter()
        .map(|e| match e {
            PathEl::ClosePath => 'Z',
            PathEl::CurveTo(..) => 'C',
            PathEl::LineTo(..) => 'L',
            PathEl::MoveTo(..) => 'M',
            PathEl::QuadTo(..) => 'Q',
        })
        .collect()
}

/// Hackery to support debugging; it's useful to see the groups
pub fn nth_group_color(n: usize) -> (u8, u8, u8) {
    // Taken from https://m2.material.io/design/color/the-color-system.html#tools-for-picking-colors
    // "2014 Material Design color palettes"
    const COLORS: &[(u8, u8, u8)] = &[
        (0xEF, 0x53, 0x50),
        (0xEC, 0x40, 0x7A),
        (0xAB, 0x47, 0xBC),
        (0xE5, 0x39, 0x35),
        (0xD8, 0x1B, 0x60),
        (0x8E, 0x24, 0xAA),
        (0xC6, 0x28, 0x28),
        (0xAD, 0x14, 0x57),
        (0x6A, 0x1B, 0x9A),
    ];

    COLORS[n % COLORS.len()]
}

#[cfg(test)]
mod tests {}
