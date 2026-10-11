//! Current-format demonstration and stress corpus tests.

use locus_syntax::parse;
use std::fs;
use std::path::PathBuf;

#[test]
fn parses_every_current_format_example() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut paths = Vec::new();
    for group in ["demonstrations", "stress", "composition"] {
        for entry in fs::read_dir(corpus.join(group))? {
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
    for path in paths {
        let source = fs::read_to_string(&path)?;
        let output = parse(&source);
        assert!(
            output.diagnostics.is_empty(),
            "{} produced diagnostics:\n{:#?}",
            path.display(),
            output.diagnostics
        );
    }
    Ok(())
}
