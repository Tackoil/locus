//! Render the repository's banner from its checked-in Locus source.

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

    let metrics = LayoutMetrics::default();
    let (inputs, measured) =
        measure_file(&parsed.file, &PlaintextEngine::new(), &MockEngine, metrics)?;
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
    let svg = render_svg(
        resolved,
        &placed,
        &styles,
        RenderOptions {
            background: Some(Color(255, 255, 255, 255)),
            ..RenderOptions::default()
        },
    )?;
    print!("{svg}");
    Ok(())
}
