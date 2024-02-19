//! A representation for simple animation of font glyphs

use kurbo::{BezPath, Rect, Shape};
use ordered_float::OrderedFloat;
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawError, DrawSettings},
};

use crate::{
    a_contained_point,
    animator::{Animated, IntervalPosition},
    error::AnimationError,
    path_commands,
    shape_pen::SubPathPen,
    GlyphSpec,
};

/// An animated glyph in no particular output format and with no specific motion curve
///
/// Meant to support simple animation patterns that are applicable to all icons such as changing
/// position in designspace over time.
#[derive(Debug, Clone)]
pub struct AnimatedGlyph {
    pub font_drawbox: Rect,

    /// Groups paths that should be animated together
    ///
    /// By default, only one entry with all the paths. Call
    /// group_icon_parts to break apart groups.
    pub contents: Group,
    grouped: bool,
}

#[derive(Debug, Clone)]
pub enum Element {
    /// Some formats (Android Vector Drawable) prefer to animate groups over paths
    /// so we put animated transformation on group only
    Group(Group),
    Path(Animated<BezPath>),
}

#[derive(Debug, Clone, Default)]
pub struct Group {
    pub rotate: Option<Animated<f64>>,
    pub uniform_scale: Option<Animated<f64>>,
    pub children: Vec<Element>,
}

fn paths_at(glyph_spec: &GlyphSpec<'_>, location: LocationRef) -> Result<Vec<BezPath>, DrawError> {
    let mut subpath_pen = SubPathPen::default();
    let settings = DrawSettings::unhinted(Size::unscaled(), location);
    glyph_spec.glyph.draw(settings, &mut subpath_pen)?;
    Ok(subpath_pen.paths())
}

impl TryFrom<GlyphSpec<'_>> for AnimatedGlyph {
    type Error = AnimationError;

    fn try_from(glyph_spec: GlyphSpec<'_>) -> Result<Self, Self::Error> {
        let start_paths =
            paths_at(&glyph_spec, (&glyph_spec.start).into()).map_err(Self::Error::DrawError)?;

        // Maybe there is an ending outline, and if there is there might be intermediary stops too
        let paths: Vec<_> = if let Some(end_loc) = glyph_spec.end.as_ref() {
            let end_paths =
                paths_at(&glyph_spec, end_loc.into()).map_err(Self::Error::DrawError)?;

            let start_cmds = start_paths
                .iter()
                .map(|bez| path_commands(bez))
                .collect::<Vec<_>>()
                .join("\n");
            let end_cmds = end_paths
                .iter()
                .map(|bez| path_commands(bez))
                .collect::<Vec<_>>()
                .join("\n");

            // TODO: figure out where to swap shapes if start/end aren't compatible
            // In theory you could swap several times such that start and end are compatible but there are swaps between. Don't care.
            assert!(start_cmds == end_cmds);
            eprintln!(
                "OMG, we have {} start shapes and {} end shapes. Compatible? {}",
                start_paths.len(),
                end_paths.len(),
                start_cmds == end_cmds
            );

            start_paths
                .into_iter()
                .zip(end_paths.into_iter())
                .map(|(start, end)| {
                    Animated::new(vec![
                        (IntervalPosition::START, start),
                        (IntervalPosition::END, end),
                    ])
                })
                .collect::<Result<_, _>>()?
        } else {
            start_paths
                .into_iter()
                .map(|bez| Animated::start_only(bez))
                .collect()
        };

        let contents = Group {
            children: paths.into_iter().map(|p| Element::Path(p)).collect(),
            ..Default::default()
        };

        Ok(Self {
            font_drawbox: glyph_spec.drawbox(),
            contents,
            grouped: false,
        })
    }
}

struct LeafGroupIter<'a> {
    frontier: Vec<&'a mut Group>,
}

impl<'a> Iterator for LeafGroupIter<'a> {
    type Item = &'a mut Group;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.frontier.pop() {
            if next.children.iter().all(|c| matches!(c, Element::Path(..))) {
                return Some(next);
            }
            self.frontier
                .extend(next.children.iter_mut().filter_map(|c| match c {
                    Element::Group(g) => Some(g),
                    Element::Path(..) => None,
                }));
        }
        None
    }
}

impl AnimatedGlyph {
    /// Walk across the leafmost groups.
    ///
    /// Intended use is to first [`group_for_piecewise_animation`] then walk leaf
    /// groups to apply animation that should be piece by piece.
    pub fn leaf_groups_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut Group> {
        LeafGroupIter {
            frontier: vec![&mut self.contents],
        }
    }

    /// The root group, to which global transformation like spinning the whole glyph, could be applied.

    /// Piece-wise animation wants to animate "parts" as the eye perceives them; try to so group.
    ///
    /// The glyph must have one group containing only paths, if not the grouping fails. Upon successful
    /// completion the glyph contents will be a Group of Group of Paths.
    ///
    /// Most importantly, if we have a shape and hole(s) cut out of it they should be together.
    ///
    /// Make some simplifying assumptions:
    ///
    /// 1. Icons don't typically use one subpath to cut a hole in many other subpaths
    /// 1. Icons typically fully contain the holepunch within the ... punchee?
    ///
    /// Since we are using non-zero fill, figure out shape by shape what the winding value is. Initially I thought
    /// we could simply look at the direction from [`BezPath::area`] but that ofc isn't enough to know if the final
    /// winding is nonzero.
    ///
    /// We do all the grouping on the initial state of animated paths.
    pub fn group_for_piecewise_animation(&mut self) -> Result<(), AnimationError> {
        // nop if already done
        if self.grouped {
            return Ok(());
        }

        let paths: Vec<_> = std::mem::take(&mut self.contents)
            .children
            .into_iter()
            .map(|e| match e {
                Element::Path(p) => Ok(p),
                Element::Group { .. } => Err(AnimationError::NotAGroupOfPaths),
            })
            .collect::<Result<_, _>>()?;

        // Figure out what is/isn't filled
        let filled: Vec<_> = paths
            .iter()
            .map(|subpath| {
                let bez = subpath.first();
                let Some(contained) = a_contained_point(&bez) else {
                    if bez.area() != 0.0 {
                        eprintln!("THERE IS NO CONTAINED POINT?! {}", bez.to_svg());
                    }
                    return false;
                };
                let winding: i32 = paths
                    .iter()
                    .map(|subpath| subpath.first().winding(contained))
                    .sum();
                winding != 0
            })
            .collect();

        // Sort filled ahead of unfilled, smaller before larger (to simplify matching below)
        let mut ordered: Vec<_> = (0..paths.len()).collect();
        ordered.sort_by_cached_key(|i| {
            (
                -(filled[*i] as i32),
                OrderedFloat(paths[*i].first().bounding_box().area()),
            )
        });

        // Group cutouts with the smallest containing filled subpath
        // Doesn't generalize but perhaps suffices for icons
        // In each group [0] must exist and is a filled subpath, [1..n] are optional and are unfilled
        let mut grouped_paths: Vec<Vec<Animated<BezPath>>> = Default::default();
        let mut bboxes = Vec::default(); // the bbox of group[n][0] is bbox[n]
        for i in ordered {
            let path = &paths[i];
            let bbox = path.first().bounding_box();
            if filled[i] {
                // start a new group for a filled subpath
                grouped_paths.push(vec![path.clone()]);
                bboxes.push(bbox);
            } else {
                // add cutout to the smallest (first, courtesy of sort above) containing filled subpath
                if let Some(i) = bboxes
                    .iter()
                    .position(|group_bbox| group_bbox.intersect(bbox) == bbox)
                {
                    grouped_paths[i].push(path.clone());
                } else {
                    eprintln!(
                        "Uh oh, we have an unfilled shape that didn't land anywhere! {}",
                        path.first().to_svg()
                    );
                }
            }
        }

        self.contents = Group {
            children: grouped_paths
                .into_iter()
                .map(|paths| {
                    Element::Group(Group {
                        children: paths.into_iter().map(|p| Element::Path(p)).collect(),
                        ..Default::default()
                    })
                })
                .collect(),
            ..Default::default()
        };
        self.grouped = true;
        Ok(())
    }
}
