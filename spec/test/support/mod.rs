//! Shared workload contracts for the CLI and renderer stress tests.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use locus_syntax::{File, Statement};

/// A named workload and the minimum amount of work it must still exercise.
pub struct Case {
    /// File stem in the stress corpus.
    pub name: &'static str,
    min_objects: usize,
    min_assertions: usize,
    min_equations: usize,
    min_attempts: usize,
}

/// Locate the stress corpus from either consuming workspace crate.
pub fn stress_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/test/stress")
}

impl Case {
    /// Source file corresponding to this workload.
    pub fn path(&self) -> PathBuf {
        stress_root().join(format!("{}.locus", self.name))
    }

    /// Parse the workload and reject accidental reductions to a tiny example.
    pub fn parse(&self) -> Result<File, Box<dyn Error>> {
        let parsed = locus_syntax::parse(&fs::read_to_string(self.path())?);
        assert!(
            parsed.diagnostics.is_empty(),
            "{}: {:?}",
            self.name,
            parsed.diagnostics
        );
        assert!(
            parsed.diagnostics.is_empty(),
            "{}: expected a valid locus 0 beta 5 file",
            self.name
        );
        let count = parsed
            .file
            .statements
            .iter()
            .filter(|stmt| matches!(stmt, Statement::Op(_)))
            .count();
        assert!(
            count >= self.min_objects,
            "{}: {count} objects, expected at least {}",
            self.name,
            self.min_objects
        );
        Ok(parsed.file)
    }

    /// Check evaluated workload size and convergence without a machine-speed threshold.
    pub fn check_report(
        &self,
        assertions: usize,
        equations: usize,
        attempts: usize,
        residual: f64,
    ) {
        assert!(
            assertions >= self.min_assertions,
            "{}: too few assertions ({assertions})",
            self.name
        );
        assert!(
            equations >= self.min_equations,
            "{}: too few equations ({equations})",
            self.name
        );
        assert!(
            attempts >= self.min_attempts,
            "{}: relational workload must require iterations",
            self.name
        );
        assert!(
            residual.is_finite() && residual < 1e-12,
            "{}: residual {residual}",
            self.name
        );
    }
}

fn case(line: &'static str) -> Result<Case, Box<dyn Error>> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(format!("invalid stress workload row: {line}").into());
    }
    Ok(Case {
        name: fields[0],
        min_objects: fields[1].parse()?,
        min_assertions: fields[2].parse()?,
        min_equations: fields[3].parse()?,
        min_attempts: fields[4].parse()?,
    })
}

/// Load explicit stress contracts and ensure every top-level example is registered exactly once.
pub fn cases() -> Result<Vec<Case>, Box<dyn Error>> {
    let cases = include_str!("../stress/stress.tsv")
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(case)
        .collect::<Result<Vec<_>, _>>()?;
    let mut registered = cases.iter().map(Case::path).collect::<Vec<_>>();
    registered.sort();
    let mut discovered = fs::read_dir(stress_root())?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    discovered.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "locus")
    });
    discovered.sort();
    assert!(
        !discovered.is_empty(),
        "the stress corpus must not be empty"
    );
    assert_eq!(
        registered, discovered,
        "update stress.tsv when adding or removing an example"
    );
    Ok(cases)
}
