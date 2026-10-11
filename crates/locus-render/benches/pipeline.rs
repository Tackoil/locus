//! Performance benchmark for a representative Locus pipeline.

// Criterion generates public harness functions that cannot carry source-level docs.
#![allow(missing_docs)]

use std::error::Error;
use std::hint::black_box;
use std::path::Path;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use locus_core::{evaluate_assertions_with_inputs, solve_with_inputs};
use locus_layout::{LayoutOptions, measurement::measure_file, solve_layout};
use locus_render::{RenderOptions, render_svg, style::StyleSheet};
use locus_syntax::{File, parse};
use locus_tex::{PlaintextEngine, RexEngine};

const SOURCE: &str = include_str!("../../../spec/test/stress/tangent_internal_branches.locus");

fn parse_source(source: &str) -> Result<File, Box<dyn Error>> {
    let parsed = parse(source);
    if parsed.diagnostics.is_empty() {
        Ok(parsed.file)
    } else {
        Err(format!(
            "benchmark fixture has diagnostics: {:?}",
            parsed.diagnostics
        )
        .into())
    }
}

fn render_source(source: &str) -> Result<String, Box<dyn Error>> {
    let file = parse_source(source)?;
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
    let resolved = scene.resolved().ok_or("accepted benchmark scene")?;
    let assertions = evaluate_assertions_with_inputs(&file, resolved, &scene.inputs)?;
    if !assertions.passed {
        return Err("benchmark fixture failed its assertions".into());
    }
    let styles = StyleSheet::from_file(&scene.file)?;
    Ok(render_svg(
        resolved,
        &placed,
        &styles,
        RenderOptions::default(),
    )?)
}

fn benchmark_pipeline(criterion: &mut Criterion) {
    let parsed = parse(SOURCE);
    assert!(
        parsed.diagnostics.is_empty(),
        "benchmark fixture has diagnostics: {:?}",
        parsed.diagnostics
    );
    let file = parsed.file;
    let measured = measure_file(
        &file,
        &PlaintextEngine::new(),
        &RexEngine,
        locus_core::preset::LayoutMetrics::default(),
    );
    assert!(
        measured.is_ok(),
        "benchmark text measurements: {measured:?}"
    );
    let Ok((inputs, _)) = measured else { return };
    assert!(
        render_source(SOURCE).is_ok(),
        "benchmark fixture must complete the rendering pipeline"
    );

    let mut group = criterion.benchmark_group("tangent_internal_branches");
    group.bench_function("parse", |bencher| {
        bencher.iter(|| parse(black_box(SOURCE)));
    });
    group.bench_function("solve", |bencher| {
        bencher.iter(|| solve_with_inputs(black_box(&file), black_box(&inputs)));
    });
    group.bench_function("end_to_end_svg", |bencher| {
        bencher.iter(|| render_source(black_box(SOURCE)));
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("../../target/criterion"))
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5));
    targets = benchmark_pipeline
}
criterion_main!(benches);
