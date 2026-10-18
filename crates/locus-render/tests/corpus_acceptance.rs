//! Real typography and SVG/TikZ/PNG acceptance for the complete beta-six corpus.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use locus_core::{EvaluationInputs, MeasuredRole, evaluate_assertions_with_inputs};
use locus_layout::{LayoutOptions, measurement::measure_file, solve_layout};
use locus_render::{RenderOptions, render_svg, render_tikz, style::StyleSheet};
use locus_tex::{PlaintextEngine, RexEngine};
use serde_json::{Value, json};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths = Vec::new();
    for group in ["examples/demonstrations", "examples/stress"] {
        for entry in fs::read_dir(root().join(group))? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "locus")
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn measured(
    path: &Path,
) -> Result<
    (
        locus_syntax::File,
        EvaluationInputs,
        locus_layout::TextBoxes,
    ),
    Box<dyn Error>,
> {
    let parsed = locus_syntax::parse(&fs::read_to_string(path)?);
    assert!(
        parsed.diagnostics.is_empty(),
        "{}: {:?}",
        path.display(),
        parsed.diagnostics
    );
    let (inputs, boxes) = measure_file(
        &parsed.file,
        &PlaintextEngine::new(),
        &RexEngine,
        locus_core::preset::LayoutMetrics::default(),
    )?;
    Ok((parsed.file, inputs, boxes))
}

fn fixture_values(inputs: &EvaluationInputs) -> Value {
    inputs
        .measured
        .iter()
        .map(|(id, text)| {
            let role = match text.role {
                MeasuredRole::Normal => "normal",
                MeasuredRole::Title => "title",
            };
            (
                id.clone(),
                json!({"width": text.width, "height": text.height, "width_bits": text.width.to_bits(), "height_bits": text.height.to_bits(), "role": role}),
            )
        })
        .collect()
}

#[test]
fn core_corpus_measurement_fixture_matches_real_font_engines() -> Result<(), Box<dyn Error>> {
    let mut fixtures = serde_json::Map::new();
    for path in sources()? {
        let (_, inputs, _) = measured(&path)?;
        fixtures.insert(
            path.strip_prefix(root())?.to_string_lossy().into_owned(),
            fixture_values(&inputs),
        );
    }
    let actual =
        json!({"engine": "bundled PlaintextEngine and RexEngine", "measurements": fixtures});
    let actual = format!("{}\n", serde_json::to_string_pretty(&actual)?);
    let path = root().join("tests/fixtures/text_measurements.json");
    if std::env::var_os("LOCUS_UPDATE_MEASUREMENT_FIXTURE").is_some() {
        fs::write(&path, &actual)?;
    }
    let expected = fs::read_to_string(path)?;
    assert_eq!(
        actual, expected,
        "core fixture must retain real bundled font measurements"
    );
    Ok(())
}

fn render_source(path: &Path) -> Result<(), Box<dyn Error>> {
    let (file, inputs, measured) = measured(path)?;
    let options = LayoutOptions {
        em_size: inputs.metrics.em_size,
        ..LayoutOptions::default()
    };
    let (scene, placed) = solve_layout(&file, inputs, &measured, options)?;
    let resolved = scene.resolved().ok_or("accepted scene")?;
    let assertions = evaluate_assertions_with_inputs(&file, resolved, &scene.inputs)?;
    if !assertions.passed {
        return Err(format!("assertions failed: {assertions:?}").into());
    }
    let styles = StyleSheet::from_file(&scene.file)?;
    let options = RenderOptions::default();
    let svg = render_svg(resolved, &placed, &styles, options)?;
    let document = roxmltree::Document::parse(&svg)?;
    assert_eq!(document.root_element().tag_name().name(), "svg");
    let tikz = render_tikz(resolved, &placed, &styles, options)?;
    vector_parity(&document, &tikz)?;
    #[cfg(feature = "png")]
    raster_parity(
        &document,
        &locus_render::png::render(resolved, &placed, &styles, options)?,
    )?;
    export_vectors(path, &svg, &tikz)?;
    #[cfg(feature = "png")]
    export_png(path, resolved, &placed, &styles)?;
    Ok(())
}

fn output_file(source: &Path, extension: &str) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let Some(output) = std::env::var_os("LOCUS_CORPUS_OUTPUT_DIR") else {
        return Ok(None);
    };
    let relative = source.strip_prefix(root())?;
    let category = if relative.starts_with("examples/demonstrations") {
        "demonstrations"
    } else {
        "stress"
    };
    let directory = Path::new(&output).join(category);
    fs::create_dir_all(&directory)?;
    let name = source.file_stem().ok_or("corpus source filename")?;
    Ok(Some(directory.join(name).with_extension(extension)))
}

fn export_vectors(source: &Path, svg: &str, tikz: &str) -> Result<(), Box<dyn Error>> {
    if let Some(path) = output_file(source, "svg")? {
        fs::write(path, svg)?;
    }
    if let Some(path) = output_file(source, "tex")? {
        fs::write(path, tikz)?;
    }
    Ok(())
}

#[cfg(feature = "png")]
fn export_png(
    source: &Path,
    scene: &locus_core::ResolvedScene,
    layout: &locus_layout::LayoutScene,
    styles: &StyleSheet,
) -> Result<(), Box<dyn Error>> {
    let Some(path) = output_file(source, "png")? else {
        return Ok(());
    };
    let options = RenderOptions {
        background: Some(locus_render::style::Color(255, 255, 255, 255)),
        ..RenderOptions::default()
    };
    fs::write(
        path,
        locus_render::png::render_with_dpi(scene, layout, styles, options, 300)?,
    )?;
    Ok(())
}

fn vector_parity(document: &roxmltree::Document<'_>, tikz: &str) -> Result<(), Box<dyn Error>> {
    let groups: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-source-start").is_some())
        .collect();
    let scopes: Vec<_> = tikz.split("  % source ").skip(1).collect();
    assert!(
        !groups.is_empty(),
        "corpus drawing must contain visible geometry"
    );
    assert_eq!(
        groups.len(),
        scopes.len(),
        "vector backends must draw the same operations"
    );
    for (group, scope) in groups.into_iter().zip(scopes) {
        let span = format!(
            "{}..{}",
            group
                .attribute("data-source-start")
                .ok_or("SVG source start")?,
            group.attribute("data-source-end").ok_or("SVG source end")?
        );
        assert_eq!(
            scope.lines().next(),
            Some(span.as_str()),
            "source operation order must agree"
        );
        let shapes: Vec<_> = group
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        let paths = scope
            .lines()
            .filter(|line| line.trim_start().starts_with("\\path["))
            .count();
        assert_eq!(
            shapes.len(),
            paths,
            "{} primitive count must agree",
            group.attribute("id").unwrap_or("operation")
        );
        for shape in shapes {
            if let Some(data) = shape.attribute("d").map(tikz_path).transpose()?.flatten() {
                assert!(
                    scope.contains(&format!("] {data};")),
                    "{} path coordinates must agree",
                    group.attribute("id").unwrap_or("path")
                );
            }
        }
    }
    Ok(())
}

fn tikz_point(x: f64, y: f64) -> String {
    format!(
        "({},{})",
        locus_render::svg::format_float(x),
        locus_render::svg::format_float(y)
    )
}

fn tikz_path(data: &str) -> Result<Option<String>, Box<dyn Error>> {
    let mut commands = Vec::new();
    for segment in svgtypes::PathParser::from(data) {
        let Some(command) = tikz_command(segment?)? else {
            return Ok(None);
        };
        commands.push(command);
    }
    Ok(Some(commands.join(" ")))
}

fn tikz_command(segment: svgtypes::PathSegment) -> Result<Option<String>, Box<dyn Error>> {
    use svgtypes::PathSegment;
    let command = match segment {
        PathSegment::MoveTo { abs: true, x, y } => tikz_point(x, y),
        PathSegment::LineTo { abs: true, x, y } => format!("-- {}", tikz_point(x, y)),
        PathSegment::CurveTo {
            abs: true,
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => format!(
            ".. controls {} and {} .. {}",
            tikz_point(x1, y1),
            tikz_point(x2, y2),
            tikz_point(x, y)
        ),
        PathSegment::ClosePath { .. } => "-- cycle".to_owned(),
        // Native arcs use equivalent radius/angle commands in TikZ.
        PathSegment::EllipticalArc { .. } => return Ok(None),
        _ => return Err("unexpected command in serialized SVG path".into()),
    };
    Ok(Some(command))
}

#[cfg(feature = "png")]
fn raster_parity(document: &roxmltree::Document<'_>, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(bytes)?;
    let svg = document.root_element();
    let width: f64 = svg.attribute("width").ok_or("SVG width")?.parse()?;
    let height: f64 = svg.attribute("height").ok_or("SVG height")?.parse()?;
    assert!((f64::from(pixmap.width()) - width).abs() <= 1.0);
    assert!((f64::from(pixmap.height()) - height).abs() <= 1.0);
    assert!(
        pixmap.data().chunks_exact(4).any(|pixel| pixel[3] > 0),
        "corpus PNG must contain visible ink"
    );
    Ok(())
}

#[test]
fn vector_and_raster_parity_cover_mixed_geometry_and_text() -> Result<(), Box<dyn Error>> {
    for name in ["rounded_rect", "bspline_controls", "point_relations"] {
        render_source(&root().join(format!("examples/demonstrations/{name}.locus")))?;
    }
    Ok(())
}

#[test]
fn function_class_parameters_style_expanded_vector_and_raster_paths() -> Result<(), Box<dyn Error>>
{
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\nclass blue_dash { accent: #2563EB, stroke: dash(12, 8), weight: 4 }\npoint start() { mark: none }\npoint finish = on_circle(circle(start, 6), 0deg) { mark: none }\nfn connector(a, b, stroke_style) {\n    segment path(a, b) [stroke_style]\n}\nconnector signal(start, finish, blue_dash)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (inputs, measured) = measure_file(
        &parsed.file,
        &PlaintextEngine::new(),
        &RexEngine,
        locus_core::preset::LayoutMetrics::default(),
    )?;
    let (scene, placed) = solve_layout(&parsed.file, inputs, &measured, LayoutOptions::default())?;
    let resolved = scene.resolved().ok_or("accepted function style scene")?;
    let styles = StyleSheet::from_file(&scene.file)?;
    let options = RenderOptions::default();
    let svg = render_svg(resolved, &placed, &styles, options)?;
    let document = roxmltree::Document::parse(&svg)?;
    let stroke = document
        .descendants()
        .find(|node| node.attribute("stroke").is_some())
        .ok_or("styled function stroke")?;
    assert_eq!(stroke.attribute("stroke"), Some("#2563EB"));
    assert_eq!(
        stroke.attribute("stroke-dasharray"),
        Some("12.000000 8.000000")
    );
    let tikz = render_tikz(resolved, &placed, &styles, options)?;
    assert!(tikz.contains("draw={rgb,255:red,37;green,99;blue,235}"));
    assert!(tikz.contains("dash pattern=on 9.000000pt off 6.000000pt"));
    #[cfg(feature = "png")]
    assert_blue_dashed_raster(&locus_render::png::render(
        resolved, &placed, &styles, options,
    )?)?;
    Ok(())
}

#[cfg(feature = "png")]
fn assert_blue_dashed_raster(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(bytes)?;
    assert!(pixmap.pixels().iter().any(|pixel| {
        pixel.alpha() == 255 && pixel.red() == 37 && pixel.green() == 99 && pixel.blue() == 235
    }));
    let row = pixmap
        .data()
        .chunks_exact(pixmap.width() as usize * 4)
        .max_by_key(|row| row.chunks_exact(4).filter(|pixel| pixel[3] > 0).count())
        .ok_or("raster stroke row")?;
    let mut runs = 0;
    let mut previous = false;
    for pixel in row.chunks_exact(4) {
        let visible = pixel[3] > 0;
        if visible && !previous {
            runs += 1;
        }
        previous = visible;
    }
    assert!(runs >= 3, "PNG must retain multiple colored dash runs");
    Ok(())
}

fn render_group(group: &str, expected: usize) -> Result<(), Box<dyn Error>> {
    let mut failures = Vec::new();
    let mut paths = sources()?;
    paths.retain(|path| path.starts_with(root().join(group)));
    assert_eq!(
        paths.len(),
        expected,
        "every {group} source must participate"
    );
    for path in paths {
        eprintln!("acceptance: {}", path.display());
        if let Err(error) = render_source(&path) {
            eprintln!("acceptance failure: {}: {error}", path.display());
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

#[test]
fn all_demonstrations_render_with_shared_measurements() -> Result<(), Box<dyn Error>> {
    render_group("examples/demonstrations", 43)
}

#[test]
fn all_stress_sources_render_with_shared_measurements() -> Result<(), Box<dyn Error>> {
    render_group("examples/stress", 14)
}
