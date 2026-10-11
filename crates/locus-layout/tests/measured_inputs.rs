//! Layout preserves the solver's accepted text and route bounds.

use std::error::Error;

use locus_core::{EvaluationInputs, MeasuredRole, MeasuredText, Scene};
use locus_layout::{LayoutOptions, TextBoxes, layout};
use locus_tex::{BoxMetrics, TexBox};

#[test]
fn standalone_label_keeps_its_solved_center_and_measured_extent() -> Result<(), Box<dyn Error>> {
    let parsed = locus_syntax::parse(
        "locus 0 beta 6\nrect box(4, 2)\nlabel caption(\"Ready\")\nrel center(caption, box)\n",
    );
    assert!(parsed.diagnostics.is_empty());
    let mut inputs = EvaluationInputs::default();
    inputs.measured.insert(
        "caption".to_owned(),
        MeasuredText {
            width: 2.0,
            height: 1.0,
            role: MeasuredRole::Normal,
        },
    );
    let scene = Scene::with_inputs(parsed.file, inputs)?;
    let resolved = scene.resolved().ok_or("resolved scene")?;
    let mut measured = TextBoxes::new();
    measured.insert(
        "caption".to_owned(),
        TexBox {
            metrics: BoxMetrics::new(2.0, 1.0, 0.0),
            advance: 2.0,
            ..TexBox::default()
        },
    );
    let placed = layout(resolved, &measured, LayoutOptions::default())?;
    let expected = resolved
        .objects
        .iter()
        .find(|(id, _)| id.name == "caption")
        .and_then(|(_, object)| object.bbox())
        .ok_or("caption bounds")?;
    assert_eq!(placed.annotations["caption"].bounds, expected);
    assert_eq!(placed.annotations["caption"].candidate_index, 0);
    assert!((expected.width() - 2.0).abs() < 1.0e-12);
    assert!((expected.height() - 1.0).abs() < 1.0e-12);
    Ok(())
}

#[test]
fn orthogonal_layout_retains_solved_route_and_curved_layout_retains_solved_control()
-> Result<(), Box<dyn Error>> {
    for kind in ["orthogonal", "curved"] {
        let parsed = locus_syntax::parse(&format!(
            "locus 0 beta 6\npoint start()\npoint finish = offset(start, 0, 3)\narrow path(start, finish) {{ route: {kind}, side: right }}\nrel path.length == 7\n"
        ));
        assert!(parsed.diagnostics.is_empty());
        let scene = Scene::new(parsed.file)?;
        let resolved = scene.resolved().ok_or("resolved route")?;
        let route = resolved
            .objects
            .values()
            .find_map(|object| match object {
                locus_core::ResolvedObject::Annotation(locus_core::AnnotationSpec::Arrow {
                    route: Some(route),
                    ..
                }) => Some(route),
                _ => None,
            })
            .ok_or("accepted route")?;
        let placed = layout(resolved, &TextBoxes::new(), LayoutOptions::default())?;
        match &placed.annotations["path"].leader {
            locus_layout::Leader::Routed {
                route: accepted, ..
            } => assert_eq!(accepted, route),
            locus_layout::Leader::Arrow {
                from, to, control, ..
            } => {
                assert_eq!(*from, route.from.clone().into());
                assert_eq!(*to, route.to.clone().into());
                assert_eq!(*control, route.control.clone().map(Into::into));
            }
            _ => return Err("missing routed leader".into()),
        }
        let expected = route.bbox();
        let actual = placed.annotations["path"].bounds;
        assert!((actual.width() - expected.width() - 0.04).abs() < 1e-12);
        assert!((actual.height() - expected.height() - 0.04).abs() < 1e-12);
    }
    Ok(())
}
