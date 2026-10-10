//! Local failure archives, compiled only into debug/dev CLI builds.

use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::Command;
use crate::exit::Exit;
use crate::output::{self, CliDiagnostic, CommandOutput, Execution};

thread_local! {
    static REQUEST: RefCell<Option<Request>> = const { RefCell::new(None) };
}

#[derive(Serialize)]
struct Request {
    schema_version: u8,
    timestamp_unix_nanos: String,
    cli_version: &'static str,
    arguments: Vec<String>,
    working_directory: Option<PathBuf>,
    original_files: Vec<OriginalFile>,
    stdin: Option<String>,
    candidate_sources: Vec<Source>,
}

#[derive(Serialize)]
struct OriginalFile {
    path: PathBuf,
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    read_error: Option<String>,
}

#[derive(Serialize)]
struct Source {
    path: PathBuf,
    source: String,
}

#[derive(Serialize)]
struct Record<'a> {
    #[serde(flatten)]
    request: Request,
    exit_code: u8,
    error_names: Vec<&'a str>,
    diagnostics: &'a [CliDiagnostic],
    result: &'a CommandOutput,
    evaluated_sources: Vec<Source>,
}

impl Request {
    fn new(paths: &[PathBuf]) -> Self {
        Self {
            schema_version: 1,
            timestamp_unix_nanos: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_string(),
            cli_version: env!("CARGO_PKG_VERSION"),
            arguments: std::env::args_os()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect(),
            working_directory: std::env::current_dir().ok(),
            original_files: paths.iter().map(|path| original_file(path)).collect(),
            stdin: None,
            candidate_sources: Vec::new(),
        }
    }
}

fn original_file(path: &Path) -> OriginalFile {
    let mut file = OriginalFile {
        path: path.to_owned(),
        source: None,
        source_bytes: None,
        read_error: None,
    };
    match fs::read(path) {
        Ok(bytes) => decode_source(&mut file, bytes),
        Err(error) => file.read_error = Some(error.to_string()),
    }
    file
}

fn decode_source(file: &mut OriginalFile, bytes: Vec<u8>) {
    match String::from_utf8(bytes) {
        Ok(source) => file.source = Some(source),
        Err(error) => file.source_bytes = Some(error.into_bytes()),
    }
}

pub(crate) fn begin(command: &Command) {
    let paths = match command {
        Command::Init(args) => vec![args.file.clone()],
        Command::Add(args)
        | Command::Describe(args)
        | Command::Inspect(args)
        | Command::Fmt(args) => {
            vec![args.file.clone()]
        }
        Command::Update(args) => vec![args.file.clone()],
        Command::Remove(args) => vec![args.file.clone()],
        Command::Render(args) => vec![args.file.clone()],
        Command::Check(args) => args.files.clone(),
    };
    REQUEST.with(|request| *request.borrow_mut() = Some(Request::new(&paths)));
}

pub(crate) fn stdin(source: &str) {
    REQUEST.with(|request| {
        if let Some(request) = request.borrow_mut().as_mut() {
            request.stdin = Some(source.to_owned());
        }
    });
}

pub(crate) fn candidate(path: &Path, source: &str) {
    REQUEST.with(|request| {
        if let Some(request) = request.borrow_mut().as_mut() {
            request.candidate_sources.push(Source {
                path: path.to_owned(),
                source: source.to_owned(),
            });
        }
    });
}

pub(crate) fn save(execution: &Execution) {
    let request = REQUEST.with(|request| request.borrow_mut().take());
    if execution.exit == Exit::Success {
        return;
    }
    if let Some(request) = request {
        archive(request, execution);
    }
}

pub(crate) fn save_usage(message: &str) {
    let paths: Vec<_> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "locus")
        })
        .collect();
    archive(Request::new(&paths), &output::usage_error(message));
}

fn diagnostics(execution: &Execution) -> Vec<CliDiagnostic> {
    match &execution.output {
        CommandOutput::Failure(failure) => failure.diagnostics.clone(),
        CommandOutput::Check(check) => check
            .files
            .iter()
            .filter(|file| !file.passed)
            .flat_map(|file| {
                let mut diagnostics = file.diagnostics.clone();
                diagnostics.extend(output::assertion_diagnostics(file));
                diagnostics
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn archive(request: Request, execution: &Execution) {
    let diagnostics = diagnostics(execution);
    let mut error_names: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    error_names.sort_unstable();
    error_names.dedup();
    let record = Record {
        request,
        exit_code: execution.exit.code(),
        error_names,
        diagnostics: &diagnostics,
        result: &execution.output,
        evaluated_sources: execution
            .sources
            .iter()
            .map(|source| Source {
                path: PathBuf::from(&source.path),
                source: source.source.clone(),
            })
            .collect(),
    };
    if let Err(error) = write_record(&record) {
        eprintln!("warning: could not save rejected request: {error}");
    }
}

fn write_record(record: &Record<'_>) -> io::Result<()> {
    let directory = std::env::var_os("LOCUS_REJECTED_REQUESTS_DIR")
        .map_or_else(|| PathBuf::from(".locus/rejected-requests"), PathBuf::from);
    let bytes = serde_json::to_vec_pretty(record)?;
    fs::create_dir_all(&directory)?;
    for sequence in 0_u32.. {
        let path = directory.join(format!(
            "{}-{}-{sequence}.json",
            record.request.timestamp_unix_nanos,
            std::process::id(),
        ));
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                file.write_all(&bytes)?;
                return file.write_all(b"\n");
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "rejected-request filename space exhausted",
    ))
}
