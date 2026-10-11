//! Command-line entry point for Locus.

mod commands;
mod compact;
mod exit;
mod output;
#[cfg(debug_assertions)]
mod rejected_requests;

use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use crate::commands::execute;
use crate::exit::Exit;

#[derive(Debug, Parser)]
#[command(
    name = "locus",
    version,
    about = "Deterministic geometry diagrams as code"
)]
struct Cli {
    /// Emit stable machine-readable output on stdout.
    #[arg(long, global = true)]
    json: bool,

    /// Emit concise single-line JSON, with check summaries and failed assertions only.
    #[arg(long, global = true, requires = "json")]
    compact: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new Locus scene file.
    Init(InitArgs),
    /// Append stdin to a scene and validate the complete file.
    Add(FileArgs),
    /// Surgically change one declaration and re-solve.
    Update(UpdateArgs),
    /// Remove one declaration when no dependents would be orphaned.
    Remove(RemoveArgs),
    /// Report declarations, resolved geometry, and solver details.
    Describe(FileArgs),
    /// Report diagnostics and requested measurements or incidences.
    Inspect(InspectArgs),
    /// Solve, inspect, and evaluate every in-file assertion.
    Check(CheckArgs),
    /// Canonically format source or export the parsed AST.
    Fmt(FileArgs),
    /// Render a scene as SVG, PNG, or `TikZ`.
    Render(RenderArgs),
}

#[derive(Debug, Args)]
struct FileArgs {
    /// Locus source file.
    file: PathBuf,
}

#[derive(Debug, Args)]
struct InspectArgs {
    /// Locus source file.
    file: PathBuf,
    /// Include exhaustive pairwise distances, triangle angles, and line ratios.
    #[arg(long, conflicts_with_all = ["distance", "angle", "ratio"])]
    measurements: bool,
    /// Include all automatically detected geometric incidences.
    #[arg(long)]
    incidences: bool,
    /// Measure a distance; repeat for additional pairs.
    #[arg(long, num_args = 2, value_names = ["FIRST", "SECOND"], action = clap::ArgAction::Append)]
    distance: Vec<String>,
    /// Measure an angle in radians; the middle point is the vertex. Repeat as needed.
    #[arg(long, num_args = 3, value_names = ["FIRST", "VERTEX", "SECOND"], action = clap::ArgAction::Append)]
    angle: Vec<String>,
    /// Measure a line ratio; the middle point must be strictly between the endpoints.
    #[arg(long, num_args = 3, value_names = ["START", "POINT", "END"], action = clap::ArgAction::Append)]
    ratio: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Style {
    #[default]
    Textbook,
}

#[derive(Debug, Args)]
struct InitArgs {
    /// New Locus source file.
    file: PathBuf,
    /// Initial rendering style.
    #[arg(long, value_enum, default_value_t)]
    style: Style,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("edit")
        .required(true)
        .multiple(false)
        .args(["definition", "property"])
))]
struct UpdateArgs {
    /// Locus source file.
    file: PathBuf,
    /// Declaration identifier to update.
    #[arg(long)]
    id: String,
    /// Replacement declaration right-hand side.
    #[arg(long = "def", value_name = "RHS", allow_hyphen_values = true)]
    definition: Option<String>,
    /// Property assignment in key=value form.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    property: Option<String>,
}

#[derive(Debug, Args)]
struct RemoveArgs {
    /// Locus source file.
    file: PathBuf,
    /// Declaration identifier to remove.
    #[arg(long)]
    id: String,
}

#[derive(Debug, Args)]
struct CheckArgs {
    /// One or more Locus source files.
    #[arg(required = true)]
    files: Vec<PathBuf>,
    /// Treat inspection warnings as failures.
    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct RenderArgs {
    /// Locus source file.
    file: PathBuf,
    /// Output path.
    #[arg(short, long, required_unless_present = "stdout")]
    output: Option<PathBuf>,
    /// Request PNG output (requires the png build feature).
    #[cfg(feature = "png")]
    #[arg(long, conflicts_with = "tikz")]
    png: bool,
    /// PNG resolution in dots per inch (default: 96).
    #[cfg(feature = "png")]
    #[arg(long, requires = "png", value_parser = clap::value_parser!(u32).range(1..))]
    dpi: Option<u32>,
    /// Viewport background color.
    #[arg(long, value_enum, default_value_t)]
    background: Background,
    /// Request `TikZ` output.
    #[arg(long)]
    tikz: bool,
    /// Write rendered output to stdout.
    #[arg(long, conflicts_with = "json")]
    stdout: bool,
    /// Select the TeX engine; mock is reserved for deterministic tests.
    #[arg(long, value_enum, default_value_t, hide = true)]
    tex_engine: TextEngine,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum TextEngine {
    #[default]
    Rex,
    Mock,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Background {
    #[default]
    Transparent,
    White,
}

fn main() -> std::process::ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return clap_exit(&error),
    };
    #[cfg(debug_assertions)]
    rejected_requests::begin(&cli.command);
    let execution = execute(cli.command);
    #[cfg(debug_assertions)]
    rejected_requests::save(&execution);
    output::emit(&execution, cli.json, cli.compact).into()
}

fn clap_exit(error: &clap::Error) -> std::process::ExitCode {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        let _ = error.print();
        return Exit::Success.into();
    }
    #[cfg(debug_assertions)]
    rejected_requests::save_usage(&error.to_string());
    if json_requested() {
        let execution = output::usage_error(&error.to_string());
        output::emit(&execution, true, compact_requested());
    } else {
        let _ = error.print();
    }
    Exit::Usage.into()
}

fn json_requested() -> bool {
    std::env::args_os().any(|argument| argument == "--json")
}

fn compact_requested() -> bool {
    std::env::args_os().any(|argument| argument == "--compact")
}
