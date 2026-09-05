//! Command-line entry point for Locus.

mod commands;
mod exit;
mod output;

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
    /// Report measurements, incidences, and diagnostics.
    Inspect(FileArgs),
    /// Solve, inspect, and evaluate every in-file assertion.
    Check(CheckArgs),
    /// Canonically format source or export the parsed AST.
    Fmt(FileArgs),
    /// Render a scene (reserved for a later build).
    Render(RenderArgs),
}

#[derive(Debug, Args)]
struct FileArgs {
    /// Locus source file.
    file: PathBuf,
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
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Request PNG output.
    #[arg(long)]
    png: bool,
    /// Request `TikZ` output.
    #[arg(long)]
    tikz: bool,
    /// Write rendered output to stdout.
    #[arg(long)]
    stdout: bool,
}

fn main() -> std::process::ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return clap_exit(&error),
    };
    let execution = execute(cli.command);
    output::emit(&execution, cli.json);
    execution.exit.into()
}

fn clap_exit(error: &clap::Error) -> std::process::ExitCode {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        let _ = error.print();
        return Exit::Success.into();
    }
    if json_requested() {
        let execution = output::usage_error(&error.to_string());
        output::emit(&execution, true);
    } else {
        let _ = error.print();
    }
    Exit::Usage.into()
}

fn json_requested() -> bool {
    std::env::args_os().any(|argument| argument == "--json")
}
