//! Last-iterate, per-element discrete retries under the shared candidate cap.
#![allow(clippy::expect_used, reason = "search integration diagnostics")]
use locus_core::{Scene, evaluate_assertions};

fn scene(source: &str) -> Scene {
    let parsed = locus_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let scene = Scene::new(parsed.file).expect("discrete search solve");
    assert!(
        evaluate_assertions(
            &scene.source_file,
            scene.resolved().expect("accepted scene")
        )
        .expect("search assertions")
        .passed
    );
    scene
}

#[test]
fn multiple_avoidance_pairs_retain_different_forced_escape_directions() {
    let scene = scene(
        "locus 0 beta 6\npoint origin()\nnum dx = free 0\nnum dy = free 0\npoint shifted = offset(origin,dx,dy)\nrect first(origin,4,1)\nrect second(shifted,4,1)\npoint other = origin\nnum ux = free 0\nnum uy = free 0\npoint other_shifted = offset(other,ux,uy)\nrect third(other,1,4)\nrect fourth(other_shifted,1,4)\nrel align_center_y(first,second)\nrel align_center_x(third,fourth)\nrel avoid(first,second)\nrel avoid(third,fourth)\nassert avoid(first,second)\nassert avoid(third,fourth)\nassert align_center_y(first,second)\nassert align_center_x(third,fourth)\n",
    );
    let summary = scene.solve_summary().expect("search report");
    assert!(summary.outer_search_attempts > 1);
    assert!(summary.outer_search_attempts <= locus_core::preset::SEARCH_CAP);
    let escapes: Vec<_> = summary
        .branch_resolutions
        .iter()
        .filter(|branch| branch.choice.starts_with("avoidance escape"))
        .collect();
    assert_eq!(escapes.len(), 2);
    assert_ne!(escapes[0].choice, escapes[1].choice);
}

#[test]
fn routing_and_avoidance_choices_are_retried_in_one_joint_candidate() {
    let scene = scene(
        "locus 0 beta 6\npoint origin()\nnum dx = free 0\nnum dy = free 0\npoint shifted = offset(origin,dx,dy)\ncircle first(origin,1)\ncircle second(shifted,1)\npoint finish = offset(origin,3,2)\narrow path(origin,finish) { route: orthogonal }\nrel path.height == 4\nrel path.width == 3\nrel path.length == 9\nrel align_center_y(first,second)\nrel avoid(first,second)\nassert path.height == 4\nassert path.width == 3\nassert path.length == 9\nassert avoid(first,second)\n",
    );
    let summary = scene.solve_summary().expect("joint search report");
    assert!(summary.outer_search_attempts > 1);
    assert!(summary.outer_search_attempts <= locus_core::preset::SEARCH_CAP);
    assert!(
        summary
            .branch_resolutions
            .iter()
            .any(|branch| branch.choice.contains("Above"))
    );
}
