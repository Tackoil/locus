//! Shared measured glyphs and their exact solve-time placements.
use std::{collections::BTreeMap, fmt::Write as _};

use locus_core::{
    AnnotationSpec, BBox, EvaluationInputs, Point, ResolvedObject, ResolvedScene, trace,
};
use locus_layout::{LabelBox, LayoutScene, Leader, PlacedAnnotation, TextBoxes};
use locus_render::{RenderOptions, render_svg, style::StyleSheet, viewport::Viewport};
use locus_tex::{BoxMetrics, TexBox};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(super) struct Measurement {
    id: String,
    width: f64,
    height: f64,
    role: String,
    #[serde(default)]
    outline: Option<Outline>,
}

#[derive(Serialize, Deserialize)]
struct Outline {
    svg: String,
    width: f64,
    ascent: f64,
    depth: f64,
    baseline_x: f64,
    em_size: f64,
}

pub(super) fn measurements(inputs: &EvaluationInputs, boxes: &TextBoxes) -> Vec<Measurement> {
    boxes
        .iter()
        .filter_map(|(id, text)| {
            let measured = inputs.measured.get(id)?;
            Some(Measurement {
                id: id.clone(),
                width: measured.width,
                height: measured.height,
                role: format!("{:?}", measured.role),
                outline: Some(Outline {
                    svg: format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\">{}</svg>",
                        text.svg_body
                    ),
                    width: text.metrics.width,
                    ascent: text.metrics.height,
                    depth: text.metrics.depth,
                    baseline_x: text.baseline_x,
                    em_size: inputs.metrics.em_size,
                }),
            })
        })
        .collect()
}

pub(super) struct Previews {
    measurements: BTreeMap<String, Measurement>,
    styles: StyleSheet,
}

impl Previews {
    pub(super) fn new(source: &str, steps: &[trace::Step]) -> Self {
        let data = steps
            .iter()
            .find(|step| step.name == "measure_text")
            .and_then(|step| step.data.get("measurements"))
            .cloned()
            .unwrap_or_default();
        let measurements: Vec<Measurement> = serde_json::from_value(data).unwrap_or_default();
        let expanded = locus_syntax::expand(&locus_syntax::parse(source).file);
        Self {
            measurements: measurements
                .into_iter()
                .map(|m| (m.id.clone(), m))
                .collect(),
            styles: StyleSheet::from_file(&expanded.file).unwrap_or_default(),
        }
    }

    pub(super) fn render(&self, scene: &ResolvedScene) -> Option<String> {
        let mut layout = LayoutScene {
            visible_paths: BTreeMap::new(),
            annotations: indexmap::IndexMap::new(),
            geometry_bbox: scene.bbox,
            bbox: scene.bbox,
            diagnostics: Vec::new(),
        };
        for (id, object) in &scene.objects {
            let ResolvedObject::Annotation(
                spec @ AnnotationSpec::Label {
                    bounds: Some(bounds),
                    ..
                },
            ) = object
            else {
                continue;
            };
            let Some(outline) = self
                .measurements
                .get(&id.name)
                .and_then(|m| m.outline.as_ref())
            else {
                continue;
            };
            layout
                .annotations
                .insert(id.name.clone(), placed(id, spec, *bounds, outline));
            layout.bbox = layout.bbox.union(*bounds);
        }
        super::arrows::append(scene, &mut layout);
        let options = RenderOptions::default();
        let view = Viewport::fit(scene, &layout, &self.styles, options).ok()?;
        let mut svg = render_svg(scene, &layout, &self.styles, options).ok()?;
        let mut labels = String::new();
        for annotation in layout.annotations.values() {
            let Some(text) = annotation.text.as_ref() else {
                continue;
            };
            let outline = self
                .measurements
                .get(&annotation.id.name)?
                .outline
                .as_ref()?;
            let origin = view.to_svg(Point::new(
                text.baseline.x - outline.baseline_x * text.em_size,
                text.baseline.y,
            ));
            let style = self.styles.for_object(&annotation.id.name);
            if style.opacity == 0.0 {
                continue;
            }
            append_label(
                &mut labels,
                &annotation.id.name,
                origin,
                text.em_size * view.pixels_per_unit,
                &style,
            );
        }
        svg = svg.replacen("</svg>", &format!("{labels}</svg>"), 1);
        Some(svg)
    }
}

fn placed(
    id: &locus_syntax::Ident,
    spec: &AnnotationSpec,
    bounds: BBox,
    outline: &Outline,
) -> PlacedAnnotation {
    PlacedAnnotation {
        id: id.clone(),
        spec: ResolvedObject::Annotation(spec.clone()),
        bounds,
        text: Some(LabelBox {
            bounds,
            baseline: Point::new(
                bounds.min.x + outline.baseline_x * outline.em_size,
                bounds.max.y - outline.ascent * outline.em_size,
            ),
            // Glyphs are shared once in measure_text and expanded by the viewer.
            measured: TexBox::new(
                "",
                BoxMetrics::new(outline.width, outline.ascent, outline.depth),
            ),
            em_size: outline.em_size,
        }),
        leader: Leader::None,
        strokes: Vec::new(),
        candidate_index: 0,
    }
}

fn append_label(
    svg: &mut String,
    id: &str,
    origin: Point,
    scale: f64,
    style: &locus_render::style::Style,
) {
    let id = escape(id);
    let _ = write!(
        svg,
        "<g id=\"{id}\" fill=\"{}\" opacity=\"{}\"><g data-measured-label=\"{id}\" transform=\"translate({} {}) scale({scale})\"/></g>",
        style.accent.hex(),
        style.opacity * style.accent.opacity(),
        origin.x,
        origin.y
    );
}

pub(super) fn gallery(measurements: &[Measurement]) -> Option<String> {
    if measurements.is_empty() {
        return None;
    }
    let scale = RenderOptions::default().pixels_per_unit;
    let width = measurements
        .iter()
        .map(|m| m.width * scale + 48.0)
        .fold(400.0, f64::max);
    let mut body = String::new();
    let mut y = 20.0;
    for measured in measurements {
        let Some(outline) = &measured.outline else {
            continue;
        };
        let _ = write!(
            body,
            "<text x=\"24\" y=\"{y}\" font-size=\"12\" fill=\"#475569\">{} · {:.3} × {:.3}</text>",
            escape(&measured.id),
            measured.width,
            measured.height
        );
        y += 12.0;
        let _ = write!(
            body,
            "<rect x=\"24\" y=\"{y}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#0891b2\" stroke-dasharray=\"3 2\"/>",
            measured.width * scale,
            measured.height * scale
        );
        append_label(
            &mut body,
            &measured.id,
            Point::new(24.0, y + outline.ascent * outline.em_size * scale),
            outline.em_size * scale,
            &locus_render::style::Style::default(),
        );
        y += measured.height * scale + 32.0;
    }
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" data-measurement-gallery=\"true\" width=\"{width}\" height=\"{y}\" viewBox=\"0 0 {width} {y}\">{body}</svg>"
    ))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
