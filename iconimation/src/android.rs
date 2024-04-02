//! Produce an output suitable for Android, e.g. an AnimatedVectorDrawable, from an Animation

use kurbo::{BezPath, Point};

use crate::{
    error::AndroidError,
    ir::{self, FromAnimation},
};

/// An in memory representation of an [AndroidVectorDrawable](https://developer.android.com/reference/android/graphics/drawable/AnimatedVectorDrawable)
///
/// Limited to capabilities needed for icon animation. Can emit a [[single-file](https://developer.android.com/reference/android/graphics/drawable/AnimatedVectorDrawable#define-an-animatedvectordrawable-all-in-one-xml-file)
/// representation for use in Android projects.
#[derive(Debug)]
pub struct AnimatedVectorDrawable {
    width: f64,
    height: f64,
    drawable: Group,
}

impl FromAnimation for AnimatedVectorDrawable {
    type Err = AndroidError;

    fn from_animation(animation: &crate::ir::Animation) -> Result<Self, Self::Err> {
        Ok(AnimatedVectorDrawable {
            width: animation.width,
            height: animation.height,
            drawable: to_avd_group(&animation.root),
        })
    }
}

fn start_el(xml: &mut String, depth: u32, name: &str, attrs: Vec<&str>) {
    for _ in 0..(depth * 2) {
        xml.push(' ');
    }
    xml.push('<');
    xml.push_str(name);
    if !attrs.is_empty() {
        xml.push('\n');
    }
    for (i, attr) in attrs.iter().enumerate() {
        write_attr(xml, depth, attr);
        if i + 1 < attrs.len() {
            xml.push('\n');
        }
    }
    xml.push_str(">\n");
}

fn end_el(xml: &mut String, depth: u32, name: &str) {
    for _ in 0..(depth * 2) {
        xml.push(' ');
    }
    xml.push_str("</");
    xml.push_str(name);
    xml.push_str(">\n");
}

fn write_attr(xml: &mut String, depth: u32, content: &str) {
    for _ in 0..(depth * 2 + 4) {
        xml.push(' ');
    }
    xml.push_str(content);
}

impl AnimatedVectorDrawable {
    /// Writes an AnimatedVectorDrawable in xml format
    ///
    /// The namespaces are tiresome with serde, just do it by hand for now
    pub fn to_avd_xml(&self) -> Result<String, AndroidError> {
        let mut xml = String::new();
        start_el(
            &mut xml,
            0,
            "animated-vector",
            vec![
                r#"xmlns:android="http://schemas.android.com/apk/res/android""#,
                r#"xmlns:aapt="http://schemas.android.com/aapt""#,
            ],
        );

        start_el(&mut xml, 1, r#"aapt:attr name="android:drawable""#, vec![]);
        eprint!("What width/height?");
        start_el(
            &mut xml,
            2,
            "vector",
            vec![
                &format!("android:width=\"{}dp\"", 24),
                &format!("android:height=\"{}dp\"", 24),
                &format!("android:viewportWidth=\"{}\"", self.width),
                &format!("android:viewportHeight=\"{}\"", self.height),
            ],
        );
        self.drawable.to_avd_xml(&mut xml, 3)?;
        end_el(&mut xml, 2, "vector");
        end_el(&mut xml, 1, "aapt:attr");

        xml.push_str("\n   <!-- TODO: animated state -->\n\n");

        end_el(&mut xml, 0, "animated-vector");
        Ok(xml)
    }
}

/// <https://developer.android.com/develop/ui/views/graphics/vector-drawable-resources#vector-drawable-class>
/// suggests clip-path as well but we don't currently use that
#[derive(Debug)]
pub(crate) enum Element {
    Group(Group),
    Path(Path),
}

impl Element {
    fn to_avd_xml(&self, xml: &mut String, depth: u32) -> Result<(), AndroidError> {
        match self {
            Element::Group(g) => g.to_avd_xml(xml, depth),
            Element::Path(p) => p.to_avd_xml(xml, depth),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Group {
    children: Vec<Element>,
    _pivot: Point,
}

impl Group {
    fn to_avd_xml(&self, xml: &mut String, depth: u32) -> Result<(), AndroidError> {
        start_el(xml, depth, "group", vec![]);
        for el in &self.children {
            el.to_avd_xml(xml, depth + 1)?;
        }
        end_el(xml, depth, "group");
        Ok(())
    }
}

fn to_avd_group(group: &ir::Group) -> Group {
    let mut children = Vec::with_capacity(group.children.len());
    for i in 0..group.children.len() {
        let next = &group.children[i];
        match next {
            ir::Element::Group(g) => children.push(Element::Group(to_avd_group(g))),
            ir::Element::Shape(s) => {
                if let Some(Element::Path(p)) = children.last_mut() {
                    // glue paths back together because unlike Lottie independent AVD paths do *not* cut holes in each other
                    p.path += &s.earliest().value.to_svg();
                } else {
                    children.push(Element::Path(to_avd_path(group.fill, s)));
                }
            }
        }
    }
    Group {
        _pivot: group.center,
        children,
    }
}

#[derive(Debug)]
pub(crate) struct Path {
    fill: String,
    path: String,
}

impl Path {
    fn to_avd_xml(&self, xml: &mut String, depth: u32) -> Result<(), AndroidError> {
        start_el(
            xml,
            depth,
            "path",
            vec![
                &format!("android:fillColor=\"{}\"", self.fill),
                &format!("android:pathData=\"{}\"", self.path),
            ],
        );
        end_el(xml, depth, "path");
        Ok(())
    }
}

fn to_avd_path(fill: Option<(u8, u8, u8)>, shape: &ir::Keyframed<BezPath>) -> Path {
    let initial_state = &shape.earliest().value;
    Path {
        fill: fill
            .map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"))
            .unwrap_or(String::from("#000000")),
        path: initial_state.to_svg(),
    }
}
