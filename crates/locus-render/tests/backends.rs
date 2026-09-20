//! Full-scene golden tests and backend parity regressions.
#![allow(clippy::expect_used, clippy::float_cmp)]

use indexmap::IndexMap;
use locus_core::scene::LabelDirection;
use locus_core::{
    AnchorSpec, AnnotationSpec, Arc, BBox, Circle, Line, Point, PointMark, Ray, ResolvedArc,
    ResolvedObject, ResolvedPoint, ResolvedScene, Segment,
    geom::{Bspline, Ellipse},
};
use locus_layout::{LabelBox, LayoutScene, Leader, PlacedAnnotation};
use locus_render::{
    RenderError, RenderOptions, render_svg, render_tikz,
    style::{Color, StyleSheet},
    viewport::Viewport,
};
use locus_syntax::{HeadKind, Ident, Span, TextValue};
use locus_tex::{BoxMetrics, FontSize, PlaintextEngine, TexBox, TexEngine};

fn id(name: &str) -> Ident {
    Ident {
        name: name.into(),
        span: Span::new(2, 1, 8, 24),
    }
}
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn bounds(a: Point, b: Point) -> BBox {
    BBox { min: a, max: b }
}
fn segment(a: Point, b: Point) -> ResolvedObject {
    ResolvedObject::Segment(Segment { start: a, end: b })
}
fn styles(source: &str) -> StyleSheet {
    let source = source.replacen(
        "locus 0 beta 4\n",
        "locus 0 beta 4\npoint a = point()\npoint b = point()\npoint c = point()\n",
        1,
    );
    let parsed = locus_syntax::parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    StyleSheet::from_file(&parsed.file).expect("styles resolve")
}
fn empty_layout() -> LayoutScene {
    LayoutScene {
        annotations: IndexMap::new(),
        geometry_bbox: bounds(p(0.0, 0.0), p(6.0, 4.0)),
        bbox: bounds(p(-100.0, -100.0), p(100.0, 100.0)),
        diagnostics: Vec::new(),
    }
}

fn label(name: &str, bbox: BBox, measured: TexBox) -> PlacedAnnotation {
    let baseline = p(
        bbox.min.x + measured.baseline_x,
        bbox.max.y - measured.metrics.height,
    );
    PlacedAnnotation {
        id: id(name),
        spec: AnnotationSpec::Label {
            anchor: AnchorSpec::Point(p(0.0, 0.0)),
            text: TextValue {
                segments: Vec::new(),
                span: id(name).span,
            },
            direction: LabelDirection::Center,
            gap: None,
        },
        bounds: bbox,
        text: Some(LabelBox {
            bounds: bbox,
            baseline,
            measured,
            em_size: 1.0,
        }),
        leader: Leader::None,
        strokes: Vec::new(),
        candidate_index: 0,
    }
}

fn triangle() -> (ResolvedScene, LayoutScene, StyleSheet) {
    let a = p(0.0, 0.0);
    let b = p(6.0, 0.0);
    let c = p(0.0, 4.0);
    let polygon = locus_core::geom::construct::polygon(
        vec![a, b, c],
        locus_core::Tolerance::from_bbox(&bounds(a, b)),
    )
    .expect("triangle");
    let objects = IndexMap::from([
        (id("face"), ResolvedObject::Polygon(polygon)),
        (id("a"), ResolvedObject::Point(ResolvedPoint::new(a))),
        (
            id("b"),
            ResolvedObject::Point(ResolvedPoint {
                geometry: b,
                mark: PointMark::Cross,
            }),
        ),
        (
            id("hidden"),
            ResolvedObject::Point(ResolvedPoint {
                geometry: p(1000.0, -1000.0),
                mark: PointMark::None,
            }),
        ),
        (id("ab"), segment(a, b)),
    ]);
    let scene = ResolvedScene {
        objects,
        bbox: bounds(p(0.0, -1000.0), p(1000.0, 4.0)),
    };
    let mut layout = empty_layout();
    let measured = TexBox::with_horizontal_metrics(
        "<g transform=\"scale(1 -1)\"><path d=\"M0 0 L0.5 1 L1 0 Z M0.25 0.3 Q0.5 0.5 0.75 0.3\"/></g>",
        BoxMetrics::new(1.0, 1.0, 0.0),
        0.8,
        0.2,
    );
    let mut annotation = label("caption", bounds(p(-2.0, 4.5), p(-1.0, 5.5)), measured);
    annotation.leader = Leader::Segment(Segment {
        start: c,
        end: p(-1.0, 4.5),
    });
    layout.annotations.insert("caption".into(), annotation);
    let sheet = styles(
        "locus 0 beta 4\nclass region { fill: tint, accent: #0F766E, opacity: 0.85, join: round }\nclass signal { weight: 1.5, stroke: dash(6, 3, 2), accent: #2563EB, cap: square }\npolygon face = polygon(a, b, c) [region]\nsegment ab = seg(a, b) [signal]\nlabel caption = label(a, \"A\") [highlight]\n",
    );
    (scene, layout, sheet)
}

fn circles() -> (ResolvedScene, LayoutScene, StyleSheet) {
    let circle = Circle {
        center: p(2.0, 2.0),
        radius: 2.0,
    };
    let objects = IndexMap::from([
        (id("circle"), ResolvedObject::Circle(circle)),
        (
            id("ccw"),
            ResolvedObject::Arc(ResolvedArc {
                geometry: Arc {
                    circle,
                    start: p(4.0, 2.0),
                    end: p(2.0, 0.0),
                    ccw: true,
                },
                angle_mark_count: None,
            }),
        ),
        (
            id("cw"),
            ResolvedObject::Arc(ResolvedArc {
                geometry: Arc {
                    circle,
                    start: p(4.0, 2.0),
                    end: p(2.0, 0.0),
                    ccw: false,
                },
                angle_mark_count: Some(2),
            }),
        ),
        (
            id("axis"),
            ResolvedObject::Line(Line {
                start: p(0.0, 2.0),
                through: p(4.0, 2.0),
            }),
        ),
        (
            id("ray"),
            ResolvedObject::Ray(Ray {
                start: p(2.0, 2.0),
                through: p(2.0, 3.0),
            }),
        ),
    ]);
    let scene = ResolvedScene {
        objects,
        bbox: bounds(p(0.0, 0.0), p(4.0, 4.0)),
    };
    let mut layout = empty_layout();
    let mut arrow = label("arrow", bounds(p(0.0, -1.0), p(4.0, 1.0)), TexBox::empty());
    arrow.text = None;
    arrow.spec = AnnotationSpec::Arrow {
        from: AnchorSpec::Point(p(0.0, 0.0)),
        to: AnchorSpec::Point(p(4.0, 0.0)),
        head: HeadKind::Both,
        bend: 0.0,
    };
    arrow.leader = Leader::Arrow {
        from: p(0.0, 0.0),
        to: p(4.0, 0.0),
        control: Some(p(2.0, -1.0)),
        head: HeadKind::Both,
    };
    arrow.strokes.push(Segment {
        start: p(1.0, 1.0),
        end: p(1.0, 1.5),
    });
    layout.annotations.insert("arrow".into(), arrow);
    layout.annotations.insert("formula".into(),label("formula",bounds(p(1.0,4.5),p(3.0,5.5)),TexBox::new("<g transform=\"translate(0 -0.5)\"><rect x=\"0\" y=\"0\" width=\"2\" height=\"0.05\"/><path d=\"M0 0 C0.5 -0.5 1.5 -0.5 2 0\"/></g>",BoxMetrics::new(2.0,1.0,0.0))));
    let sheet = styles(
        "locus 0 beta 4\nline axis = line(a, b) [construction]\nray ray = ray(a, b) [auxiliary]\narc ccw = arc(c, a, b) [highlight]\n",
    );
    (scene, layout, sheet)
}

#[test]
fn triangle_full_documents() {
    let (scene, layout, sheet) = triangle();
    insta::assert_snapshot!(
        "triangle_svg",
        render_svg(&scene, &layout, &sheet, RenderOptions::default()).expect("SVG")
    );
    insta::assert_snapshot!(
        "triangle_tikz",
        render_tikz(&scene, &layout, &sheet, RenderOptions::default()).expect("TikZ")
    );
}
#[test]
fn circles_full_documents() {
    let (scene, layout, sheet) = circles();
    insta::assert_snapshot!(
        "circles_svg",
        render_svg(&scene, &layout, &sheet, RenderOptions::default()).expect("SVG")
    );
    insta::assert_snapshot!(
        "circles_tikz",
        render_tikz(&scene, &layout, &sheet, RenderOptions::default()).expect("TikZ")
    );
}

#[test]
fn ellipse_and_bspline_use_native_vector_primitives() {
    let ellipse =
        Ellipse::new(p(2.0, 1.0), 2.0, 1.0, core::f64::consts::FRAC_PI_4).expect("ellipse");
    let spline = Bspline::new(vec![
        p(0.0, 0.0),
        p(1.0, 3.0),
        p(3.0, 3.0),
        p(4.0, 0.0),
        p(5.0, 2.0),
    ])
    .expect("B-spline");
    let scene = ResolvedScene {
        objects: IndexMap::from([
            (id("ellipse"), ResolvedObject::Ellipse(ellipse)),
            (id("spline"), ResolvedObject::Bspline(spline)),
        ]),
        bbox: bounds(p(0.0, 0.0), p(5.0, 3.0)),
    };
    let sheet = StyleSheet::default();
    let svg = render_svg(&scene, &empty_layout(), &sheet, RenderOptions::default()).expect("SVG");
    let tikz =
        render_tikz(&scene, &empty_layout(), &sheet, RenderOptions::default()).expect("TikZ");
    assert!(svg.contains("<ellipse") && svg.contains("transform=\"rotate("));
    assert!(svg.contains(" C "));
    assert!(tikz.contains(" ellipse[") && tikz.contains(".. controls "));
}
#[test]
fn byte_determinism_and_input_immutability() {
    for (scene, layout, sheet) in [triangle(), circles()] {
        let before = (scene.clone(), layout.clone());
        let svg = render_svg(&scene, &layout, &sheet, RenderOptions::default()).expect("SVG");
        let tikz = render_tikz(&scene, &layout, &sheet, RenderOptions::default()).expect("TikZ");
        assert_eq!(
            svg.as_bytes(),
            render_svg(&scene, &layout, &sheet, RenderOptions::default())
                .expect("SVG")
                .as_bytes()
        );
        assert_eq!(
            tikz.as_bytes(),
            render_tikz(&scene, &layout, &sheet, RenderOptions::default())
                .expect("TikZ")
                .as_bytes()
        );
        assert_eq!((scene, layout), before);
    }
}
#[test]
fn viewport_uses_visible_geometry_label_boxes_and_margin() {
    let (scene, layout, sheet) = triangle();
    let view = Viewport::fit(&scene, &layout, &sheet, RenderOptions::default()).expect("viewport");
    assert_eq!(view.bounds.min.x, -2.5);
    assert_eq!(view.bounds.max.y, 6.0);
    assert!(view.bounds.max.x < 7.0);
    assert_eq!(view.to_svg(view.bounds.min), p(0.0, view.height()));
    assert_eq!(view.to_svg(view.bounds.max), p(view.width(), 0.0));
}
#[test]
fn empty_and_flat_viewports_are_finite_and_invalid_options_fail() {
    let mut scene = ResolvedScene {
        objects: IndexMap::new(),
        bbox: bounds(p(99.0, 99.0), p(100.0, 100.0)),
    };
    let sheet = StyleSheet::default();
    let layout = empty_layout();
    let options = RenderOptions {
        margin: 0.0,
        pixels_per_unit: 1.0,
        ..RenderOptions::default()
    };
    let view = Viewport::fit(&scene, &layout, &sheet, options).expect("empty viewport");
    assert_eq!(view.width(), 1.0);
    assert_eq!(view.height(), 1.0);
    scene
        .objects
        .insert(id("s"), segment(p(0.0, 0.0), p(5.0, 0.0)));
    let view = Viewport::fit(&scene, &layout, &sheet, options).expect("flat viewport");
    assert!(view.width() > 0.0 && view.height() > 0.0);
    for (margin, pixels_per_unit) in [
        (-1.0, 1.0),
        (0.0, 0.0),
        (f64::NAN, 1.0),
        (0.0, f64::INFINITY),
    ] {
        assert_eq!(
            Viewport::fit(
                &scene,
                &layout,
                &sheet,
                RenderOptions {
                    margin,
                    pixels_per_unit,
                    ..RenderOptions::default()
                }
            ),
            Err(RenderError::Options)
        );
    }
}
#[test]
fn measured_font_outlines_use_identical_positions_in_both_backends() {
    let (scene, mut layout, sheet) = triangle();
    let measured = PlaintextEngine::default()
        .render("A&j", FontSize::DEFAULT)
        .expect("font outlines");
    let bbox = bounds(
        p(-2.0, 4.5),
        p(
            -2.0 + measured.metrics.width,
            4.5 + measured.metrics.height + measured.metrics.depth,
        ),
    );
    layout
        .annotations
        .insert("caption".into(), label("caption", bbox, measured));
    let svg = render_svg(&scene, &layout, &sheet, RenderOptions::default()).expect("SVG");
    let tikz = render_tikz(&scene, &layout, &sheet, RenderOptions::default()).expect("TikZ");
    assert!(!svg.contains("<text"));
    assert!(!tikz.contains("\\node"));
    assert!(svg.contains(" C "));
    assert!(tikz.contains(".. controls "));
}
#[test]
fn malformed_text_is_source_anchored() {
    let (scene, mut layout, sheet) = triangle();
    layout
        .annotations
        .get_mut("caption")
        .expect("caption")
        .text
        .as_mut()
        .expect("text")
        .measured
        .svg_body = "<text>A</text>".into();
    let error = render_svg(&scene, &layout, &sheet, RenderOptions::default())
        .expect_err("unsupported outline");
    assert!(matches!(error,RenderError::Invalid { span,.. } if span==id("caption").span));
    assert_eq!(error.code(), "E415");
}

#[cfg(feature = "png")]
#[test]
fn png_has_expected_dimensions_pixels_and_deterministic_bytes() {
    let (scene, layout, sheet) = triangle();
    let options = RenderOptions::default();
    let first = locus_render::png::render(&scene, &layout, &sheet, options).expect("PNG");
    let second = locus_render::png::render(&scene, &layout, &sheet, options).expect("PNG again");
    assert_eq!(first, second);
    assert_eq!(&first[..8], b"\x89PNG\r\n\x1a\n");
    let decoded = resvg::tiny_skia::Pixmap::decode_png(&first).expect("PNG decodes");
    let view = Viewport::fit(&scene, &layout, &sheet, options).expect("viewport");
    assert_eq!(f64::from(decoded.width()), libm::round(view.width()));
    assert_eq!(f64::from(decoded.height()), libm::round(view.height()));
    assert!(decoded.pixels().iter().any(|pixel| pixel.alpha() > 0));
    assert!(decoded.pixels().iter().any(|pixel| pixel.alpha() == 0));
    let reader = png::Decoder::new(first.as_slice())
        .read_info()
        .expect("metadata");
    let density = reader.info().pixel_dims.expect("print density");
    assert_eq!((density.xppu, density.yppu), (3780, 3780));
    assert_eq!(density.unit, png::Unit::Meter);
}

#[cfg(feature = "png")]
#[test]
fn png_dpi_scales_content_and_records_print_density() {
    let (scene, layout, sheet) = triangle();
    let options = RenderOptions {
        background: Some(Color(255, 255, 255)),
        ..RenderOptions::default()
    };
    let view = Viewport::fit(&scene, &layout, &sheet, options).expect("viewport");
    for dpi in [48, 192, 300] {
        let bytes =
            locus_render::png::render_with_dpi(&scene, &layout, &sheet, options, dpi).expect("PNG");
        let decoded = resvg::tiny_skia::Pixmap::decode_png(&bytes).expect("PNG decodes");
        let scale = f64::from(dpi) / 96.0;
        assert_eq!(
            f64::from(decoded.width()),
            libm::round(view.width() * scale)
        );
        assert_eq!(
            f64::from(decoded.height()),
            libm::round(view.height() * scale)
        );
        assert!(decoded.pixels().iter().all(|pixel| pixel.alpha() == 255));
        assert_eq!(decoded.pixel(0, 0).expect("corner").red(), 255);
        assert!(decoded.pixels().iter().any(|pixel| pixel.red() < 128));
        let reader = png::Decoder::new(bytes.as_slice())
            .read_info()
            .expect("metadata");
        let density = reader.info().pixel_dims.expect("print density");
        assert_eq!(density.unit, png::Unit::Meter);
        assert_eq!(density.xppu, density.yppu);
        assert!((f64::from(density.xppu) * 0.0254 - f64::from(dpi)).abs() < 0.013);
    }
}

#[cfg(feature = "png")]
#[test]
fn png_rejects_invalid_density_and_excessive_scaled_dimensions() {
    let (scene, layout, sheet) = triangle();
    for dpi in [0, 100_000, u32::MAX] {
        let error = locus_render::png::render_with_dpi(
            &scene,
            &layout,
            &sheet,
            RenderOptions::default(),
            dpi,
        )
        .expect_err("invalid PNG options");
        assert_eq!(error.code(), "E417");
    }
}

#[test]
fn optional_background_covers_the_viewport_before_content() {
    let (scene, layout, sheet) = triangle();
    let options = RenderOptions {
        background: Some(Color(255, 255, 255)),
        ..RenderOptions::default()
    };
    let svg = render_svg(&scene, &layout, &sheet, options).expect("SVG");
    let document = roxmltree::Document::parse(&svg).expect("SVG parses");
    let root = document.root_element();
    let background = root.first_element_child().expect("background");
    assert_eq!(background.tag_name().name(), "rect");
    assert_eq!(background.attribute("fill"), Some("#FFFFFF"));
    assert_eq!(background.attribute("width"), root.attribute("width"));
    assert_eq!(background.attribute("height"), root.attribute("height"));
    let tikz = render_tikz(&scene, &layout, &sheet, options).expect("TikZ");
    let fill = tikz.find("fill={rgb,255:red,255;green,255;blue,255},draw=none");
    assert!(fill.expect("background fill") < tikz.find("% source").expect("content"));
    #[cfg(feature = "png")]
    {
        let png = locus_render::png::render(&scene, &layout, &sheet, options).expect("PNG");
        let decoded = resvg::tiny_skia::Pixmap::decode_png(&png).expect("PNG decodes");
        assert!(decoded.pixels().iter().all(|pixel| pixel.alpha() == 255));
        let corner = decoded.pixel(0, 0).expect("padded corner");
        assert_eq!(
            (corner.red(), corner.green(), corner.blue()),
            (255, 255, 255)
        );
    }
}
