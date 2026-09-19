//! Render the repository logo from its checked-in Locus source.

use std::error::Error;

use locus_core::{evaluate_assertions, solve};
use locus_layout::{LayoutOptions, boxes::measure_scene, layout};
use locus_render::{RenderOptions, render_svg, style::StyleSheet};
use locus_tex::{FontSize, MockEngine, PlaintextEngine};

fn main() -> Result<(), Box<dyn Error>> {
    let parsed = locus_syntax::parse(include_str!("../../../assets/locus-logo.locus"));
    if !parsed.diagnostics.is_empty() {
        return Err(format!("logo source diagnostics: {:?}", parsed.diagnostics).into());
    }

    let solved = solve(&parsed.file)?;
    let assertions = evaluate_assertions(&parsed.file, &solved.scene)?;
    if !assertions.passed {
        return Err("logo assertions failed".into());
    }

    let measured = measure_scene(
        &solved.scene,
        &PlaintextEngine::new(),
        &MockEngine,
        FontSize::new(1.65),
    )?;
    let placed = layout(
        &solved.scene,
        &measured,
        LayoutOptions {
            gap: 0.3,
            ..LayoutOptions::default()
        },
    )?;
    let styles = StyleSheet::from_file(&parsed.file)?;
    let svg = render_svg(
        &solved.scene,
        &placed,
        &styles,
        RenderOptions {
            pixels_per_unit: 72.0,
            margin: 0.16,
            ..RenderOptions::default()
        },
    )?;
    print!("{svg}");
    Ok(())
}
