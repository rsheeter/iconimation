//! Writes an svg to help visualize motion

use clap::Parser;
use iconimation::{
    nth_group_color,
    spring::{AnimatedValue, AnimatedValueType, Spring},
    spring2cubic::cubic_approximation,
};
use std::fs;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    #[clap(default_value_t = 0.0)]
    from: f64,

    #[arg(long)]
    #[clap(default_value_t = 100.0)]
    to: f64,
}

pub fn main() {
    let args = Args::parse();
    let frame_rate = 60.0;
    let animation = AnimatedValue::new(args.from, args.to, AnimatedValueType::Scale);

    let springs = vec![
        ("standard", Spring::standard()),
        ("smooth spatial", Spring::smooth_spatial()),
        ("smooth non spatial", Spring::smooth_non_spatial()),
        ("expressive spatial", Spring::expressive_spatial()),
        ("expressive non spatial", Spring::expressive_non_spatial()),
    ];

    let mut value_seqs = Vec::new();
    for (_, spring) in springs.iter() {
        // 60fps, run until complete or 5s
        let mut frame_values = Vec::new();
        let mut animated_value = animation;
        for frame in 0..300 {
            let time = frame as f64 / frame_rate;
            animated_value = spring.update(time, animated_value);
            frame_values.push(animated_value);
            if animated_value.is_at_equilibrium() {
                break;
            }
        }
        assert!(
            frame_values.len() < 300,
            "Should finish within 300 frames\n{frame_values:#?}"
        );
        value_seqs.push(frame_values);
    }

    // convert time to frames so the span on each axis is somewhat similar, plots better
    let (time_extent, value_extent) = value_seqs
        .iter()
        .flat_map(|values| {
            values.iter().map(|v| {
                (
                    (v.time * frame_rate, v.time * frame_rate),
                    (v.value, v.value),
                )
            })
        })
        .reduce(|(acc_time, acc_value), (e_time, e_value)| {
            (
                (acc_time.0.min(e_time.0), acc_time.1.max(e_time.1)),
                (acc_value.0.min(e_value.0), acc_value.1.max(e_value.1)),
            )
        })
        .unwrap();

    let mut svg = String::new();
    let time_span = time_extent.1 - time_extent.0;
    let value_span = value_extent.1 - value_extent.0;
    let time_margin = 0.1 * time_span;
    let value_margin = 0.1 * value_span;

    svg.push_str(&format!("<svg viewBox=\"{:.2} {:.2} {:.2} {:.2}\" version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" >\n",
        time_extent.0 - time_margin,
        value_extent.0 - value_margin,
        time_span + 2.0 * time_margin,
        value_span + 2.0 * value_margin));

    for (i, values) in value_seqs.iter().enumerate() {
        let name = springs[i].0;
        svg.push_str(&format!("\n  <!-- {name} -->\n"));
        let (r, g, b) = nth_group_color(i * 2);
        let color = format!("#{r:02x}{g:02x}{b:02x}");
        for value in values {
            svg.push_str(&format!(
                "  <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"0.25\" fill=\"{color}\" />\n",
                value.time * frame_rate,
                value.value,
            ));
        }

        let (name, spring) = springs[i];
        let cubics = cubic_approximation(frame_rate, animation, spring).expect(name);
        svg.push_str(&format!(
            "<path fill=\"none\" stroke=\"{color}\" stroke-width=\"0.2\" d=\"\n"
        ));
        svg.push_str(&format!("  M{:.2},{:.2}\n", cubics[0].p0.x, cubics[0].p0.y));
        for cubic in cubics {
            svg.push_str(&format!(
                "  C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\n",
                cubic.p1.x, cubic.p1.y, cubic.p2.x, cubic.p2.y, cubic.p3.x, cubic.p3.y
            ));
        }
        svg.push_str("\" />\n");

        svg.push_str(&format!(
            "  <text x=\"{}\" y=\"{}\" font-size=\"4\" fill=\"{color}\">{name}</text>\n",
            time_margin + time_span / 3.0,
            value_extent.0 + 5.0 * i as f64
        ));
    }
    svg.push_str("</svg>\n");

    let filename = "/tmp/curves.svg";
    fs::write(filename, svg).expect("write");
    println!("Wrote {filename}");
}
