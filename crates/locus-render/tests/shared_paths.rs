//! Shared path decoration renders identically across finite path constructors.
use locus_core::EvaluationInputs;
use locus_layout::{LayoutOptions, TextBoxes, solve_layout};
use locus_render::{RenderOptions, render_svg, render_tikz, style::StyleSheet};

fn render(
    kind: &str,
    head: &str,
    clipped: bool,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let clip = if clipped {
        "rel trim_endpoints(path,boundary)\n"
    } else {
        ""
    };
    let source = format!(
        "locus 0 beta 6\npoint start() {{ mark: none }}\npoint end = on_circle(circle(start,4),0deg) {{ mark: none }}\nrect frame(start,12,12)\nrect boundary(start,2,2)\n{kind:7} path(start,end) {{ head: {head} }}\n{clip}"
    );
    let parsed = locus_syntax::parse(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let (scene, feedback) = solve_layout(
        &parsed.file,
        EvaluationInputs::default(),
        &TextBoxes::new(),
        LayoutOptions::default(),
    )?;
    let resolved = scene.resolved().ok_or("unresolved scene")?;
    let styles = StyleSheet::from_file(&scene.file)?;
    let options = RenderOptions::default();
    Ok((
        render_svg(resolved, &feedback, &styles, options)?,
        render_tikz(resolved, &feedback, &styles, options)?,
    ))
}

#[test]
fn decorated_segments_match_straight_arrows_in_svg_and_tikz()
-> Result<(), Box<dyn std::error::Error>> {
    for head in ["-", "->", "<->"] {
        for clipped in [false, true] {
            assert_eq!(
                render("segment", head, clipped)?,
                render("arrow", head, clipped)?,
                "head {head}, clipped {clipped}"
            );
        }
    }
    Ok(())
}
