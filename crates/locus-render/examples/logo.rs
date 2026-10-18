//! Render the repository's banner from its checked-in Locus source.
//!
//! SVG: `cargo run -p locus-render --example logo > assets/locus-banner.svg`
//! PNG: `cargo run -p locus-render --features png --example logo -- --png > assets/locus-banner.png`

use std::error::Error;

use locus_core::{evaluate_assertions_with_inputs, preset::LayoutMetrics};
use locus_layout::{LayoutOptions, measurement::measure_file, solve_layout};
use locus_render::{
    RenderOptions, render_svg,
    style::{Color, StyleSheet},
};
use locus_tex::{MockEngine, PlaintextEngine};

fn main() -> Result<(), Box<dyn Error>> {
    let parsed = locus_syntax::parse(include_str!("../../../assets/locus-banner.locus"));
    if !parsed.diagnostics.is_empty() {
        return Err(format!("logo source diagnostics: {:?}", parsed.diagnostics).into());
    }

    let metrics = LayoutMetrics {
        em_size: 1.6,
        ..LayoutMetrics::default()
    };
    let font = PlaintextEngine::from_static_font(
        include_bytes!("../../../assets/fonts/RobotoSerif-Regular.ttf"),
        0,
    )?;
    let (inputs, measured) = measure_file(&parsed.file, &font, &MockEngine, metrics)?;
    let wordmark = measured.get("wordmark").ok_or("missing banner wordmark")?;
    if !wordmark.svg_body.contains("<path") {
        return Err(format!(
            "Roboto Serif produced no letter outlines: {}",
            wordmark.svg_body
        )
        .into());
    }
    let (scene, placed) = solve_layout(
        &parsed.file,
        inputs,
        &measured,
        LayoutOptions {
            em_size: metrics.em_size,
            ..LayoutOptions::default()
        },
    )?;
    let resolved = scene.resolved().ok_or("logo scene was not resolved")?;
    let assertions = evaluate_assertions_with_inputs(&scene.source_file, resolved, &scene.inputs)?;
    if !assertions.passed {
        return Err("logo assertions failed".into());
    }

    let styles = StyleSheet::from_file(&scene.file)?;
    let options = RenderOptions {
        background: Some(Color(255, 255, 255, 255)),
        ..RenderOptions::default()
    };
    if std::env::args().any(|arg| arg == "--png") {
        #[cfg(feature = "png")]
        {
            use std::io::Write as _;
            let png = locus_render::png::render_with_dpi(resolved, &placed, &styles, options, 256)?;
            std::io::stdout().lock().write_all(&png)?;
        }
        #[cfg(not(feature = "png"))]
        return Err("PNG output requires --features png".into());
    } else {
        print!("{}", render_svg(resolved, &placed, &styles, options)?);
    }
    Ok(())
}
