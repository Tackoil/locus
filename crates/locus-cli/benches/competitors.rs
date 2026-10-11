//! End-to-end CLI speed comparison with Mermaid and D2.

// Criterion generates public harness functions that cannot carry source-level docs.
#![allow(missing_docs)]

use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main,
    measurement::WallTime,
};

#[derive(Clone, Copy)]
enum ToolKind {
    Locus,
    Mermaid,
    D2,
}

struct Tool {
    name: &'static str,
    kind: ToolKind,
    program: PathBuf,
}

struct Case {
    name: &'static str,
    locus: &'static str,
    mermaid: &'static str,
    d2: &'static str,
}

impl Case {
    fn input(&self, root: &Path, kind: ToolKind) -> PathBuf {
        let relative = match kind {
            ToolKind::Locus => self.locus,
            ToolKind::Mermaid => self.mermaid,
            ToolKind::D2 => self.d2,
        };
        root.join(relative)
    }
}

impl Tool {
    fn command(&self, input: &Path, output: &Path) -> Command {
        let mut command = Command::new(&self.program);
        match self.kind {
            ToolKind::Locus => {
                command.arg("render").arg(input).arg("-o").arg(output);
            }
            ToolKind::Mermaid => {
                command
                    .arg("-i")
                    .arg(input)
                    .arg("-o")
                    .arg(output)
                    .arg("-b")
                    .arg("transparent");
            }
            ToolKind::D2 => {
                command.arg(input).arg(output);
            }
        }
        command
    }

    fn render(&self, input: &Path, output: &Path) -> io::Result<()> {
        let status = self
            .command(input, output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "{} exited with {status}",
                self.name
            )))
        }
    }

    fn preflight(&self, input: &Path, output: &Path) -> io::Result<()> {
        let result = self.command(input, output).output()?;
        if !result.status.success() {
            return Err(io::Error::other(format!(
                "{} failed: {}",
                self.name,
                String::from_utf8_lossy(&result.stderr).trim()
            )));
        }
        let svg = fs::read_to_string(output)?;
        if !svg.contains("<svg") {
            return Err(io::Error::other(format!(
                "{} did not produce SVG output",
                self.name
            )));
        }
        Ok(())
    }

    fn version(&self) -> io::Result<String> {
        let result = Command::new(&self.program).arg("--version").output()?;
        let text = if result.stdout.is_empty() {
            &result.stderr
        } else {
            &result.stdout
        };
        Ok(String::from_utf8_lossy(text).trim().to_owned())
    }
}

fn configured_path(variable: &str, fallback: impl AsRef<OsStr>) -> PathBuf {
    env::var_os(variable).map_or_else(|| PathBuf::from(fallback.as_ref()), PathBuf::from)
}

fn tools(root: &Path) -> [Tool; 3] {
    [
        Tool {
            name: "locus",
            kind: ToolKind::Locus,
            program: configured_path("LOCUS_BIN", root.join("target/release/locus")),
        },
        Tool {
            name: "mermaid",
            kind: ToolKind::Mermaid,
            program: configured_path("LOCUS_MMDC", "mmdc"),
        },
        Tool {
            name: "d2",
            kind: ToolKind::D2,
            program: configured_path("LOCUS_D2", "d2"),
        },
    ]
}

fn cases() -> [Case; 2] {
    [
        Case {
            name: "agent_flowchart",
            locus: "examples/stress/flowchart_agent_loop.locus",
            mermaid: "benchmarks/competitors/agent_flowchart.mmd",
            d2: "benchmarks/competitors/agent_flowchart.d2",
        },
        Case {
            name: "software_architecture",
            locus: "examples/stress/software_architecture.locus",
            mermaid: "benchmarks/competitors/software_architecture.mmd",
            d2: "benchmarks/competitors/software_architecture.d2",
        },
    ]
}

fn stop(message: impl fmt::Display) -> ! {
    eprintln!("competitor benchmark setup failed: {message}");
    std::process::exit(2);
}

fn report_versions(tools: &[Tool]) {
    for tool in tools {
        match tool.version() {
            Ok(version) => eprintln!("{}: {version}", tool.name),
            Err(error) => stop(format!("cannot run {}: {error}", tool.name)),
        }
    }
}

fn preflight(root: &Path, output_root: &Path, tools: &[Tool], cases: &[Case]) {
    for case in cases {
        for tool in tools {
            let input = case.input(root, tool.kind);
            let output = output_root.join(format!("{}-{}.svg", case.name, tool.name));
            if let Err(error) = tool.preflight(&input, &output) {
                stop(error);
            }
        }
    }
}

fn register_case(
    group: &mut BenchmarkGroup<'_, WallTime>,
    root: &Path,
    output_root: &Path,
    tools: &[Tool],
    case: &Case,
) {
    for tool in tools {
        let input = case.input(root, tool.kind);
        let output = output_root.join(format!("{}-{}.svg", case.name, tool.name));
        group.bench_with_input(
            BenchmarkId::new(case.name, tool.name),
            &(tool, input, output),
            |bencher, (tool, input, output)| {
                bencher.iter(|| {
                    let result = tool.render(input, output);
                    assert!(result.is_ok(), "{} failed: {result:?}", tool.name);
                });
            },
        );
    }
}

fn benchmark_competitors(criterion: &mut Criterion) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_root = root.join("target/competitor-bench");
    if let Err(error) = fs::create_dir_all(&output_root) {
        stop(error);
    }
    let tools = tools(&root);
    let cases = cases();
    report_versions(&tools);
    preflight(&root, &output_root, &tools, &cases);

    let mut group = criterion.benchmark_group("cli_to_svg");
    group.sampling_mode(SamplingMode::Flat);
    for case in &cases {
        register_case(&mut group, &root, &output_root, &tools, case);
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("../../target/criterion/competitors"))
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(6));
    targets = benchmark_competitors
}
criterion_main!(benches);
