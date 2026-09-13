//! Repeat each complete stress pipeline and compare geometry, reports, layout, and output bytes.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Instant;

use locus_core::{AssertionReport, Solved, evaluate_assertions, solve};
use locus_layout::{LayoutOptions, LayoutScene, boxes::measure_scene, layout};
use locus_render::{RenderOptions, render_svg, render_tikz, style::StyleSheet};
use locus_tex::{FontSize, MockEngine, PlaintextEngine};

#[path = "../../../spec/test/support/mod.rs"]
mod support;

#[derive(Debug, PartialEq)]
struct Run {
    solved: Solved,
    assertions: AssertionReport,
    placed: LayoutScene,
    svg: String,
    tikz: String,
    #[cfg(feature = "png")]
    png: Vec<u8>,
}

fn export(name: &str, extension: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(directory) = std::env::var_os("LOCUS_EXAMPLE_OUTPUT_DIR") {
        let directory = Path::new(&directory);
        fs::create_dir_all(directory)?;
        fs::write(directory.join(format!("{name}.{extension}")), bytes)?;
    }
    Ok(())
}

fn render_case(case: &support::Case) -> Result<Run, Box<dyn Error>> {
    let file = case.parse()?;
    let solved = solve(&file)?;
    let assertions = evaluate_assertions(&file, &solved.scene)?;
    assert!(assertions.passed, "{}: {assertions:?}", case.name);
    case.check_report(
        assertions.assertions.len(),
        solved.summary.equations,
        solved.summary.attempts,
        solved.summary.max_normalized_residual,
    );
    // Original examples contain TeX; its mock boxes exercise plumbing, not real TeX rendering.
    let measured = measure_scene(
        &solved.scene,
        &PlaintextEngine::new(),
        &MockEngine,
        FontSize::DEFAULT,
    )?;
    let placed = layout(
        &solved.scene,
        &measured,
        LayoutOptions {
            em_size: 0.28,
            ..LayoutOptions::default()
        },
    )?;
    let styles = StyleSheet::from_file(&file)?;
    let options = RenderOptions::default();
    let svg = render_svg(&solved.scene, &placed, &styles, options)?;
    let document = roxmltree::Document::parse(&svg)?;
    assert_eq!(document.root_element().tag_name().name(), "svg");
    let tikz = render_tikz(&solved.scene, &placed, &styles, options)?;
    assert!(tikz.contains("\\begin{tikzpicture}"), "{}", case.name);
    #[cfg(feature = "png")]
    let png = locus_render::png::render(&solved.scene, &placed, &styles, options)?;
    #[cfg(feature = "png")]
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "{}", case.name);
    Ok(Run {
        solved,
        assertions,
        placed,
        svg,
        tikz,
        #[cfg(feature = "png")]
        png,
    })
}

fn passes() -> Result<usize, Box<dyn Error>> {
    let count =
        std::env::var("LOCUS_STRESS_PASSES").map_or(Ok(2), |value| value.parse::<usize>())?;
    if !(2..=100).contains(&count) {
        return Err("LOCUS_STRESS_PASSES must be between 2 and 100".into());
    }
    Ok(count)
}

fn repeat_case(case: &support::Case, passes: usize) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let first = render_case(case)?;
    for _ in 1..passes {
        let again = render_case(case)?;
        assert_eq!(first, again, "{}: repeated pipeline changed", case.name);
    }
    eprintln!(
        "{}: {} objects, {} asserts, {} equations, {} attempts; {passes} passes in {:?}",
        case.name,
        first.solved.scene.objects.len(),
        first.assertions.assertions.len(),
        first.solved.summary.equations,
        first.solved.summary.attempts,
        start.elapsed()
    );
    export(case.name, "svg", first.svg.as_bytes())?;
    export(case.name, "tex", first.tikz.as_bytes())?;
    #[cfg(feature = "png")]
    export(case.name, "png", &first.png)?;
    Ok(())
}

#[test]
fn stress_examples_repeat_the_complete_pipeline() -> Result<(), Box<dyn Error>> {
    let passes = passes()?;
    for case in support::cases()? {
        repeat_case(&case, passes).map_err(|error| format!("{}: {error}", case.name))?;
    }
    Ok(())
}
