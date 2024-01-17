//! An [OutlinePen] that generates an svg displaying subpaths.

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};
use ordered_float::OrderedFloat;
use skrifa::outline::OutlinePen;
use write_fonts::pens::write_to_pen;

use crate::{
    animate::{a_contained_point, group_icon_parts},
    shape_pen::SubPathPen,
};

pub struct DebugPen {
    glyph_block: Rect,
    paths: Vec<BezPath>,
}

enum MarkPoint {
    Contained(Point),
    Control(Point),
    End(Point),
}

impl MarkPoint {
    fn svg(&self) -> String {
        let (opacity, attr, point) = match self {
            MarkPoint::Contained(pt) => (100, " fill=\"red\"", *pt),
            MarkPoint::Control(pt) => (50, " fill=\"blue\"", *pt),
            MarkPoint::End(pt) => (75, "", *pt),
        };
        format!(
            "  <circle opacity=\"{opacity}%\" r=\"3\" cx=\"{}\" cy=\"{}\" {attr}/>\n",
            point.x, point.y
        )
    }
}

fn draw_annotated(svg: &mut String, y_offset: f64, mut paths: Vec<BezPath>) {
    paths.sort_by_cached_key(|p| OrderedFloat(p.area().abs()));

    svg.push_str(&format!("<g transform=\"translate(0, {y_offset})\">\n"));

    for path in &paths {
        let path_svg = path.to_svg();
        // if y_offset == 0.0 {
        //     eprintln!("{}", &path_svg[0..path_svg.find(' ').unwrap()]);
        // }

        let contained = a_contained_point(path);
        let mut filled = 0;
        if let Some(contained) = contained {
            // work out non-zero fill
            for path in &paths {
                let wind = path.winding(contained);
                filled += wind;
                // if y_offset == 0.0 {
                //     let path = path.to_svg();
                //     eprintln!(
                //         "  {} contributes {}",
                //         &path[0..path.find(' ').unwrap()],
                //         wind
                //     );
                // }
            }
        }
        let filled = filled != 0; // nonzero winding?!

        // if y_offset == 0.0 {
        //     eprintln!("  filled? {filled}");
        // }

        svg.push_str("  <path opacity=\"33%\" d=\"");
        svg.push_str(&path_svg);
        svg.push_str("  \"");
        if !filled {
            svg.push_str("\n        fill=\"none\" stroke=\"red\" stroke-dasharray=\"4\"");
        }
        svg.push_str(" />\n");

        let bbox = path.bounding_box();
        svg.push_str(&format!("  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"black\" stroke-dasharray=\"16\" />",
            bbox.min_x(), bbox.min_y(), bbox.width(), bbox.height()));

        let first_move = match path.elements().first() {
            Some(PathEl::MoveTo(p)) => format!("{p}"),
            _ => "??".to_string(),
        };

        svg.push_str(&format!(
            "  <text x=\"{}\" y=\"{}\">first {first_move} area {:.2}</text>\n",
            bbox.min_x(),
            bbox.min_y() - 2.0,
            path.area()
        ));

        let mut last_move = None;
        for el in path.elements() {
            match el {
                PathEl::MoveTo(p) => {
                    last_move = Some(*p);
                    svg.push_str(&MarkPoint::End(*p).svg());
                }
                PathEl::LineTo(p) => {
                    svg.push_str(&MarkPoint::End(*p).svg());
                }
                PathEl::QuadTo(c, p) => {
                    svg.push_str(&MarkPoint::Control(*c).svg());
                    svg.push_str(&MarkPoint::End(*p).svg());
                }
                PathEl::CurveTo(c0, c1, p) => {
                    svg.push_str(&MarkPoint::Control(*c0).svg());
                    svg.push_str(&MarkPoint::Control(*c1).svg());
                    svg.push_str(&MarkPoint::End(*p).svg());
                }
                PathEl::ClosePath => {
                    if let Some(last_move) = last_move {
                        svg.push_str(&MarkPoint::End(last_move).svg());
                    }
                }
            }
        }

        if let Some(contained) = contained {
            svg.push_str(&MarkPoint::Contained(contained).svg());
        }
    }

    svg.push_str("</g>\n");
}

impl DebugPen {
    pub fn new(glyph_block: Rect) -> DebugPen {
        DebugPen {
            glyph_block,
            paths: Default::default(),
        }
    }

    fn active_subpath(&mut self) -> &mut BezPath {
        if self.paths.is_empty() {
            self.paths.push(BezPath::new());
        }
        return self.paths.last_mut().unwrap();
    }

    pub fn to_svg(self) -> String {
        // It's nice to draw the right way up
        let transform = Affine::IDENTITY
            // Move center-y to be at y=0
            .then_translate((0.0, -self.glyph_block.center().y).into())
            // Do a flip!
            .then_scale_non_uniform(1.0, -1.0)
            // Go back again
            .then_translate((0.0, self.glyph_block.center().y).into());

        let paths: Vec<_> = self
            .paths
            .into_iter()
            .map(|mut p| {
                p.apply_affine(transform);
                p
            })
            .collect();

        let shapes = paths
            .iter()
            .flat_map(|bez| {
                let mut pen = SubPathPen::default();
                write_to_pen(bez, &mut pen);
                pen.into_shapes().into_iter()
            })
            .collect();
        let groups = group_icon_parts(shapes);

        // We need one glyph block for the annotated svg plus one per group, vertically
        let viewbox = Rect::new(
            self.glyph_block.min_x(),
            self.glyph_block.min_y(),
            self.glyph_block.max_x(),
            self.glyph_block.min_y() + self.glyph_block.height() * (1 + groups.len()) as f64,
        );

        let mut svg = format!(
            r#"<svg viewBox="{} {} {} {}""#,
            viewbox.min_x(),
            viewbox.min_y(),
            viewbox.width(),
            viewbox.height()
        );
        svg.push_str(r#" xmlns="http://www.w3.org/2000/svg""#);
        svg.push_str(">\n");

        // Draw the entire glyph annotated
        draw_annotated(&mut svg, 0.0, paths);

        // Draw each group for animation, each in it's own glyph block vertically

        for (i, group) in groups.iter().enumerate() {
            // group i draws into glyph block i+1
            let y_offset = self.glyph_block.min_y() + (i as f64 + 1.0) * self.glyph_block.height();
            let paths: Vec<_> = group.iter().map(|(bez, _)| bez.clone()).collect();
            draw_annotated(&mut svg, y_offset, paths);
        }

        svg.push_str("\n</svg>");

        svg
    }
}

impl OutlinePen for DebugPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.paths.push(BezPath::new());
        self.active_subpath().move_to((x as f64, y as f64));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.active_subpath().line_to((x as f64, y as f64));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.active_subpath()
            .quad_to((cx0 as f64, cy0 as f64), (x as f64, y as f64));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.active_subpath().curve_to(
            (cx0 as f64, cy0 as f64),
            (cx1 as f64, cy1 as f64),
            (x as f64, y as f64),
        );
    }

    fn close(&mut self) {
        self.active_subpath().close_path();
    }
}
