//! Full-scene golden tests and backend parity regressions.
#![allow(clippy::expect_used, clippy::float_cmp)]

use indexmap::IndexMap;
use locus_core::scene::LabelDirection;
use locus_core::{
    AnchorSpec, AnnotationSpec, Arc, BBox, Circle, Line, Point, PointMark, Ray, ResolvedArc,
    ResolvedObject, ResolvedPoint, ResolvedScene, Segment,
    geom::{Bspline, Ellipse},
};
use locus_core::{PathGeometry, ResolvedPath};
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
    ResolvedObject::segment(Segment { start: a, end: b })
}
fn styles(source: &str) -> StyleSheet {
    let source = source.replacen(
        "locus 0 beta 6\n",
        "locus 0 beta 6\npoint a()\npoint b()\npoint c()\n",
        1,
    );
    let parsed = locus_syntax::parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    StyleSheet::from_file(&parsed.file).expect("styles resolve")
}
fn empty_layout() -> LayoutScene {
    LayoutScene {
        visible_paths: std::collections::BTreeMap::default(),
        annotations: IndexMap::new(),
        geometry_bbox: bounds(p(0.0, 0.0), p(6.0, 4.0)),
        bbox: bounds(p(-100.0, -100.0), p(100.0, 100.0)),
        diagnostics: Vec::new(),
    }
}

#[test]
fn excessive_finite_arrow_controls_are_rejected_by_all_exporters() {
    let (scene, mut layout, sheet) = triangle();
    let mut annotation = label("arrow", bounds(p(0.0, 0.0), p(4.0, 1e17)), TexBox::empty());
    annotation.text = None;
    annotation.leader = Leader::Arrow {
        from: p(0.0, 0.0),
        to: p(4.0, 0.0),
        control: Some(p(2.0, 1e17)),
        head: HeadKind::Forward,
    };
    layout.annotations.insert("arrow".into(), annotation);
    let options = RenderOptions::default();
    for error in [
        render_svg(&scene, &layout, &sheet, options).expect_err("excessive SVG bounds"),
        render_tikz(&scene, &layout, &sheet, options).expect_err("excessive TikZ bounds"),
    ] {
        assert_eq!(error.code(), "E415");
        assert!(matches!(error, RenderError::Invalid { span, .. } if span == id("arrow").span));
        assert!(error.to_string().contains("gentler bend"));
    }
    #[cfg(feature = "png")]
    assert_eq!(
        locus_render::png::render(&scene, &layout, &sheet, options)
            .expect_err("excessive PNG bounds")
            .code(),
        "E415"
    );
}

fn label(name: &str, bbox: BBox, measured: TexBox) -> PlacedAnnotation {
    let baseline = p(
        bbox.min.x + measured.baseline_x,
        bbox.max.y - measured.metrics.height,
    );
    PlacedAnnotation {
        id: id(name),
        spec: ResolvedObject::Annotation(AnnotationSpec::Label {
            path: None,
            anchor: AnchorSpec::Point(p(0.0, 0.0)),
            text: TextValue {
                segments: Vec::new(),
                span: id(name).span,
            },
            direction: LabelDirection::Center,
            gap: None,
            bounds: None,
        }),
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
        "locus 0 beta 6\nclass region { fill: tint, accent: #0F766E, opacity: 0.85, join: round }\nclass signal { weight: 1.5, stroke: dash(6, 3, 2), accent: #2563EB, cap: square }\npolygon face(a, b, c) [region]\nsegment ab(a, b) [signal]\nlabel caption(a, \"A\") [highlight]\n",
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
    arrow.spec = ResolvedObject::Path(ResolvedPath {
        geometry: PathGeometry::Routed {
            route: None,
            from: AnchorSpec::Point(p(0.0, 0.0)),
            to: AnchorSpec::Point(p(4.0, 0.0)),
            bend: 0.0,
        },
        label_bounds: None,
        head: HeadKind::Both,
    });
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
        "locus 0 beta 6\nline axis(a, b) [construction]\nray ray(a, b) [auxiliary]\narc ccw(c, a, b, true) [highlight]\n",
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
        background: Some(Color(255, 255, 255, 255)),
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
        background: Some(Color(255, 255, 255, 255)),
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

fn alpha_scene(alpha: &str) -> (ResolvedScene, LayoutScene, StyleSheet) {
    let (scene, layout, _) = triangle();
    let parsed = locus_syntax::parse(&format!(
        "locus 0 beta 6\nclass transparent {{ accent: #FF0000{alpha}, fill: #0000FF80, opacity: 0.5 }}\npoint a() [transparent]\npoint b() [transparent]\npoint c()\npolygon face(a, b, c) [transparent]\nsegment ab(a, b) [transparent]\nlabel caption(a, \"A\") [transparent]\n"
    ));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let sheet = StyleSheet::from_file(&parsed.file).expect("styles");
    (scene, layout, sheet)
}

#[test]
fn vector_backends_preserve_stroke_fill_and_glyph_alpha() {
    let (scene, layout, sheet) = alpha_scene("40");
    let svg = render_svg(&scene, &layout, &sheet, RenderOptions::default()).expect("SVG");
    let xml = roxmltree::Document::parse(&svg).expect("XML");
    let face = xml
        .descendants()
        .find(|n| n.attribute("id") == Some("face"))
        .expect("face");
    assert_eq!(face.attribute("opacity"), Some("0.500000"));
    let path = face
        .children()
        .find(|n| n.has_tag_name("path"))
        .expect("path");
    assert_eq!(path.attribute("stroke"), Some("#FF0000"));
    assert_eq!(path.attribute("stroke-opacity"), Some("0.250980"));
    assert_eq!(path.attribute("fill-opacity"), Some("0.501961"));
    let point = xml
        .descendants()
        .find(|n| n.attribute("id") == Some("a"))
        .expect("point");
    let disk = point
        .children()
        .find(|n| n.has_tag_name("circle"))
        .expect("disk");
    assert_eq!(disk.attribute("stroke"), Some("none"));
    assert_eq!(disk.attribute("fill-opacity"), Some("0.250980"));
    let caption = xml
        .descendants()
        .find(|n| n.attribute("id") == Some("caption"))
        .expect("caption");
    assert!(
        caption
            .children()
            .any(|n| n.attribute("stroke") == Some("none")
                && n.attribute("fill-opacity") == Some("0.250980"))
    );
    let tikz = render_tikz(&scene, &layout, &sheet, RenderOptions::default()).expect("TikZ");
    assert!(tikz.contains("transparency group,opacity=0.500000"));
    assert!(tikz.contains("draw opacity=0.250980,fill opacity=0.501961"));
    assert!(tikz.contains("draw=none,fill={rgb,255:red,255;green,0;blue,0},fill opacity=0.250980"));
}

#[cfg(feature = "png")]
#[test]
fn png_composes_color_alpha_with_object_opacity() {
    let (scene, layout, sheet) = alpha_scene("00");
    let bytes =
        locus_render::png::render(&scene, &layout, &sheet, RenderOptions::default()).expect("PNG");
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&bytes).expect("PNG decodes");
    // The blue polygon interior has alpha (128 / 255) * 0.5, rounded to 64.
    assert!(
        pixmap
            .pixels()
            .iter()
            .any(|p| p.alpha() == 64 && p.blue() == 64 && p.red() == 0)
    );
    assert!(pixmap.pixels().iter().any(|p| p.alpha() == 0));
}

#[test]
fn automatic_routes_export_accepted_spans_and_controls_on_all_backends() {
    for kind in ["orthogonal", "curved"] {
        let parsed = locus_syntax::parse(&format!(
            "locus 0 beta 6\npoint start()\npoint finish = on_circle(circle(start, 3), 90deg)\narrow path(start, finish) {{ route: {kind}, side: right }}\nrel path.length == 7\n"
        ));
        assert!(parsed.diagnostics.is_empty());
        let sheet = StyleSheet::from_file(&parsed.file).expect("route style");
        let scene = locus_core::Scene::new(parsed.file).expect("solved route");
        let resolved = scene.resolved().expect("accepted route");
        let placed = locus_layout::layout(
            resolved,
            &locus_layout::TextBoxes::new(),
            locus_layout::LayoutOptions::default(),
        )
        .expect("accepted route layout");
        let svg =
            render_svg(resolved, &placed, &sheet, RenderOptions::default()).expect("routed SVG");
        let tikz =
            render_tikz(resolved, &placed, &sheet, RenderOptions::default()).expect("routed TikZ");
        assert_route_export_commands(&svg, &tikz, kind);
        if kind == "orthogonal" {
            assert!(matches!(
                placed.annotations["path"].leader,
                Leader::Routed { .. }
            ));
        } else {
            assert!(matches!(
                placed.annotations["path"].leader,
                Leader::Arrow {
                    control: Some(_),
                    ..
                }
            ));
        }
        #[cfg(feature = "png")]
        assert!(
            !locus_render::png::render(resolved, &placed, &sheet, RenderOptions::default())
                .expect("routed PNG")
                .is_empty()
        );
    }
}

fn assert_route_export_commands(svg: &str, tikz: &str, kind: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid routed SVG");
    let group = document
        .descendants()
        .find(|node| node.attribute("id") == Some("path"))
        .expect("SVG route group");
    let centerline = group
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("SVG logical centerline");
    let commands: Vec<_> =
        svgtypes::PathParser::from(centerline.attribute("d").expect("centerline path"))
            .collect::<Result<_, _>>()
            .expect("valid route commands");
    if kind == "orthogonal" {
        assert_eq!(commands.len(), 4);
        assert_eq!(
            commands
                .iter()
                .filter(|command| matches!(command, svgtypes::PathSegment::LineTo { .. }))
                .count(),
            3
        );
        assert!(tikz.lines().any(|line| line.contains("\\path[")
            && line.matches("--").count() == 3
            && !line.contains("cycle")));
    } else {
        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[1], svgtypes::PathSegment::CurveTo { .. }));
        assert!(tikz.contains(".. controls "));
    }
}

#[test]
fn clipped_arrow_and_segment_export_the_same_visible_endpoints_on_all_backends() {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\nclass hidden { opacity: 0 }\npoint start() [hidden]\npoint finish = on_circle(circle(start, 4), 0deg) [hidden]\ncircle source(start, 1)\ncircle target(finish, 1)\narrow path(start, finish)\nsegment stroke(start, finish)\ngroup paths(path, stroke)\nrel trim_endpoints(paths, source)\nrel trim_endpoints(paths, target)\n",
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let sheet = StyleSheet::from_file(&parsed.file).expect("clipping style");
    let (scene, placed) = locus_layout::solve_layout(
        &parsed.file,
        locus_core::EvaluationInputs::default(),
        &locus_layout::TextBoxes::new(),
        locus_layout::LayoutOptions::default(),
    )
    .expect("clipping feedback");
    let resolved = scene.resolved().expect("clipped scene");
    let viewport = Viewport::fit(resolved, &placed, &sheet, RenderOptions::default())
        .expect("clipped viewport");
    let retained = &placed.visible_paths["stroke"];
    let start = viewport.to_svg(retained.from.clone().into());
    let end = viewport.to_svg(retained.to.clone().into());
    let svg = render_svg(resolved, &placed, &sheet, RenderOptions::default()).expect("clipped SVG");
    let document = roxmltree::Document::parse(&svg).expect("clipped XML");
    for name in ["path", "stroke"] {
        let group = document
            .descendants()
            .find(|node| node.attribute("id") == Some(name))
            .expect("clipped path group");
        let centerline = group
            .children()
            .find(|node| node.has_tag_name("path"))
            .expect("clipped centerline");
        let commands =
            svgtypes::PathParser::from(centerline.attribute("d").expect("path commands"))
                .collect::<Result<Vec<_>, _>>()
                .expect("valid commands");
        assert_eq!(
            commands,
            vec![
                svgtypes::PathSegment::MoveTo {
                    abs: true,
                    x: serialized(start.x),
                    y: serialized(start.y)
                },
                svgtypes::PathSegment::LineTo {
                    abs: true,
                    x: serialized(end.x),
                    y: serialized(end.y)
                }
            ]
        );
    }
    let tikz =
        render_tikz(resolved, &placed, &sheet, RenderOptions::default()).expect("clipped TikZ");
    let endpoints = format!(
        "({},{}) -- ({},{})",
        locus_render::svg::format_float(start.x),
        locus_render::svg::format_float(start.y),
        locus_render::svg::format_float(end.x),
        locus_render::svg::format_float(end.y)
    );
    assert_eq!(tikz.matches(&endpoints).count(), 2);
    #[cfg(feature = "png")]
    assert!(
        !locus_render::png::render(resolved, &placed, &sheet, RenderOptions::default())
            .expect("clipped PNG")
            .is_empty()
    );
}

fn serialized(value: f64) -> f64 {
    locus_render::svg::format_float(value)
        .parse()
        .expect("serialized coordinate")
}
