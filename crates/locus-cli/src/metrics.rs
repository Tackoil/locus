//! Timing report presentation, kept separate from command output.

use std::io::{self, Write as _};

use clap::ValueEnum;
use locus_core::metrics::TimingReport;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Format {
    Human,
    Json,
}

pub(crate) fn emit(report: &TimingReport, format: Format) {
    let mut stderr = io::stderr().lock();
    let result = match format {
        Format::Human => human(&mut stderr, report),
        Format::Json => serde_json::to_writer(&mut stderr, report)
            .map_err(io::Error::other)
            .and_then(|()| writeln!(stderr)),
    };
    if let Err(error) = result {
        let _ = writeln!(stderr, "warning: could not write timings: {error}");
    }
}

fn human(writer: &mut impl io::Write, report: &TimingReport) -> io::Result<()> {
    writeln!(writer, "Timings (inclusive wall time):")?;
    let mut depths = Vec::with_capacity(report.steps.len());
    for step in &report.steps {
        let depth = step.parent.map_or(0, |parent| depths[parent] + 1);
        depths.push(depth);
        writeln!(
            writer,
            "  {:indent$}{}: {:.3} ms",
            "",
            step.name,
            step.duration_ms,
            indent = depth * 2,
        )?;
    }
    writeln!(writer, "  total: {:.3} ms", report.total_ms)
}
