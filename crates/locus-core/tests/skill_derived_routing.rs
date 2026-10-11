//! Keep the skill's executable routing example constructive and geometrically valid.

use std::error::Error;

use locus_core::{evaluate_assertions, solve};
use locus_syntax::parse;

#[test]
fn derived_routing_uses_only_one_placement_owner() -> Result<(), Box<dyn Error>> {
    let parsed = parse(include_str!(
        "../../../skills/locus/references/derived-routing.locus"
    ));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let solved = solve(&parsed.file)?;
    let assertions = evaluate_assertions(&parsed.file, &solved.scene)?;
    assert!(assertions.passed, "{assertions:?}");
    assert_eq!(assertions.assertions.len(), 8);
    assert_eq!(solved.summary.unknowns, 2);
    assert_eq!(solved.summary.equations, 0);
    assert_eq!(solved.summary.attempts, 0);
    assert_eq!(solved.summary.dof.global_remaining, 2);
    assert_eq!(solved.summary.dof.objects.len(), 1);
    assert_eq!(solved.summary.dof.objects[0].id, "client");
    Ok(())
}
