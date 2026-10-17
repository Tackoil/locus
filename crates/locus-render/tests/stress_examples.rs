//! Repeat each complete stress pipeline and compare geometry, reports, layout, and output bytes.

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::Instant;

use locus_core::{AssertionReport, Solved, evaluate_assertions_with_inputs};
use locus_layout::{LayoutOptions, LayoutScene, measurement::measure_file, solve_layout};
use locus_render::{
    RenderOptions, render_svg, render_tikz,
    style::{Color, StyleSheet},
};
use locus_tex::{PlaintextEngine, RexEngine};

#[path = "../../../tests/support/stress.rs"]
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

fn export_gallery(cases: &[support::Case]) -> Result<(), Box<dyn Error>> {
    let count = cases.len();
    let mut cards = String::new();
    for case in cases {
        let name = case.name;
        let title = name.replace('_', " ");
        let png_link = if cfg!(feature = "png") {
            format!(" · <a href=\"{name}.png\">PNG</a>")
        } else {
            String::new()
        };
        write!(
            cards,
            "<article><a href=\"{name}.svg\"><img src=\"{name}.svg\" alt=\"{title}\" loading=\"lazy\"></a><h2>{title}</h2><nav><a href=\"{name}.svg\">SVG</a>{png_link} · <a href=\"{name}.tex\">TikZ</a></nav></article>"
        )?;
    }
    let gallery = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Locus 0 beta 6 stress drawings</title><style>body{{margin:0;padding:32px;background:#f3f4f6;color:#111827;font:16px system-ui,sans-serif}}main{{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:24px}}article{{padding:16px;background:white;border:1px solid #d1d5db;border-radius:12px}}img{{display:block;width:100%;height:320px;object-fit:contain;background:white}}h1{{margin-top:0}}h2{{font-size:16px;text-transform:capitalize}}a{{color:#2563eb}}p{{color:#4b5563}}</style><h1>Locus 0 beta 6 stress drawings</h1><p>All {count} workloads, verified and drawn with a white background. Select a drawing to view the full SVG.</p><main>{cards}</main></html>"
    );
    export("index", "html", gallery.as_bytes())
}

fn render_case(case: &support::Case) -> Result<Run, Box<dyn Error>> {
    let file = case.parse()?;
    let (inputs, measured) = measure_file(
        &file,
        &PlaintextEngine::new(),
        &RexEngine,
        locus_core::preset::LayoutMetrics::default(),
    )?;
    let options = LayoutOptions {
        em_size: inputs.metrics.em_size,
        ..LayoutOptions::default()
    };
    let (scene, placed) = solve_layout(&file, inputs, &measured, options)?;
    let solved = Solved {
        scene: scene.resolved().ok_or("accepted stress scene")?.clone(),
        summary: scene.solve_summary().ok_or("stress solve report")?.clone(),
    };
    let assertions = evaluate_assertions_with_inputs(&file, &solved.scene, &scene.inputs)?;
    assert!(assertions.passed, "{}: {assertions:?}", case.name);
    case.check_report(
        assertions.assertions.len(),
        solved.summary.equations,
        solved.summary.attempts,
        solved.summary.max_normalized_residual,
    );
    let styles = StyleSheet::from_file(&scene.file)?;
    let options = RenderOptions {
        background: Some(Color(255, 255, 255, 255)),
        ..RenderOptions::default()
    };
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
    let first = measured_case(case, 1)?;
    for pass in 2..=passes {
        let again = measured_case(case, pass)?;
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

fn measured_case(case: &support::Case, pass: usize) -> Result<Run, Box<dyn Error>> {
    let start = Instant::now();
    let result = if let Some(directory) = std::env::var_os("LOCUS_STRESS_TIMINGS_DIR") {
        let (result, report) = locus_core::metrics::collect(|| render_case(case));
        let directory = Path::new(&directory);
        fs::create_dir_all(directory)?;
        fs::write(
            directory.join(format!("{}-{pass}.json", case.name)),
            serde_json::to_vec_pretty(&report)?,
        )?;
        result
    } else {
        render_case(case)
    };
    eprintln!("{}: pass {pass} in {:?}", case.name, start.elapsed());
    result
}

#[test]
fn stress_examples_repeat_the_complete_pipeline() -> Result<(), Box<dyn Error>> {
    let passes = passes()?;
    let cases = support::cases()?;
    for case in &cases {
        repeat_case(case, passes).map_err(|error| format!("{}: {error}", case.name))?;
    }
    export_gallery(&cases)?;
    Ok(())
}
