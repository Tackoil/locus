//! Thin command orchestration over `locus-syntax` and `locus-core`.
#![allow(missing_docs, reason = "command report plumbing is crate-internal")]

mod render;

use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use locus_core::{
    DependencyGraph, Dimension, EvalError, Evaluation, InspectDiagnostic, Point, PointMark,
    RemovalImpact, ResolvedObject, ResolvedScene, Scene, SolveSummary, evaluate_assertions,
    inspect_scene,
};
use locus_syntax::{Diagnostic, File, OpStmt, Span, Statement, format, parse};

use crate::exit::Exit;
use crate::output::{
    BranchReport, CheckFileResult, CheckResult, CliDiagnostic, CommandOutput, DescribeResult,
    DofReport, Execution, FailureResult, FormatResult, HintReport, InspectResult, MutationResult,
    ObjectAnchorReport, ObjectDofReport, ObjectReport, PointReport, RelationReport, ResolvedValue,
    Severity, SolveReport, SourceDocument,
};
use crate::{Command, Style};

pub(crate) fn execute(command: Command) -> Execution {
    match command {
        command
        @ (Command::Init(_) | Command::Add(_) | Command::Update(_) | Command::Remove(_)) => {
            execute_mutation(command)
        }
        command @ (Command::Describe(_) | Command::Inspect(_)) => execute_analysis(command),
        Command::Check(args) => check(&args.files, args.strict),
        Command::Fmt(args) => fmt(&args.file),
        Command::Render(args) => render::execute(&args),
    }
}

fn execute_mutation(command: Command) -> Execution {
    match command {
        Command::Init(args) => init(&args.file, args.style),
        Command::Add(args) => add(&args.file),
        Command::Update(args) => update(
            &args.file,
            &args.id,
            args.definition.as_deref(),
            args.property.as_deref(),
        ),
        Command::Remove(args) => remove(&args.file, &args.id),
        Command::Describe(_)
        | Command::Inspect(_)
        | Command::Check(_)
        | Command::Fmt(_)
        | Command::Render(_) => unreachable!("mutation dispatcher received an analysis command"),
    }
}

fn execute_analysis(command: Command) -> Execution {
    match command {
        Command::Describe(args) => describe(&args.file),
        Command::Inspect(args) => inspect(&args.file),
        Command::Init(_)
        | Command::Add(_)
        | Command::Update(_)
        | Command::Remove(_)
        | Command::Check(_)
        | Command::Fmt(_)
        | Command::Render(_) => unreachable!("analysis dispatcher received another command"),
    }
}

fn init(path: &Path, style: Style) -> Execution {
    let style = match style {
        Style::Textbook => "textbook",
    };
    let source = format!("{}\n\nstyle {style}\n", locus_syntax::FILE_HEADER);
    let result = OpenOptions::new().write(true).create_new(true).open(path);
    match result.and_then(|mut file| file.write_all(source.as_bytes())) {
        Ok(()) => mutation("init", path, None, format!("created {}", path.display())),
        Err(error) => io_failure("init", path, &error),
    }
}

fn add(path: &Path) -> Execution {
    let loaded = match read_source("add", path) {
        Ok(loaded) => loaded,
        Err(execution) => return *execution,
    };
    let mut appended = String::new();
    let read_result = io::stdin().read_to_string(&mut appended);
    #[cfg(debug_assertions)]
    crate::rejected_requests::stdin(&appended);
    if let Err(error) = read_result {
        return io_failure("add", path, &error);
    }
    let candidate = append_source(&loaded.source, &appended);
    validate_and_write("add", path, candidate, None, "appended statements")
}

fn update(path: &Path, id: &str, definition: Option<&str>, property: Option<&str>) -> Execution {
    let loaded = match load_parsed("update", path) {
        Ok(loaded) => loaded,
        Err(execution) => return *execution,
    };
    let Some(op) = find_op(&loaded.file, id) else {
        return edit_target_failure("update", &loaded, id);
    };
    let candidate = match updated_source(&loaded.source, op, definition, property) {
        Ok(candidate) => candidate,
        Err(message) => return edit_usage_failure("update", &loaded, message),
    };
    validate_and_write("update", path, candidate, Some(id), "updated declaration")
}

fn updated_source(
    source: &str,
    op: &OpStmt,
    definition: Option<&str>,
    property: Option<&str>,
) -> Result<String, &'static str> {
    if let Some(definition) = definition {
        return update_definition(source, op, definition);
    }
    if let Some(property) = property {
        return update_property(source, op, property);
    }
    Err("an update operation is required")
}

fn remove(path: &Path, id: &str) -> Execution {
    let loaded = match load_parsed("remove", path) {
        Ok(loaded) => loaded,
        Err(execution) => return *execution,
    };
    let Some(op) = find_op(&loaded.file, id) else {
        return edit_target_failure("remove", &loaded, id);
    };
    let graph = match DependencyGraph::extract(&loaded.file) {
        Ok(graph) => graph,
        Err(error) => {
            return dependency_failure("remove", &loaded, error.code, error.span, error.message);
        }
    };
    let impact = graph.removal_impact(id);
    if !impact.is_empty() {
        return orphan_failure(&loaded, op, id, &impact);
    }
    let candidate = remove_physical_line(&loaded.source, op.span);
    validate_and_write("remove", path, candidate, Some(id), "removed declaration")
}

fn orphan_failure(loaded: &LoadedFile, op: &OpStmt, id: &str, impact: &RemovalImpact) -> Execution {
    let mut dependents = impact.orphaned_declarations.clone();
    dependents.extend(impact.dependent_statements.iter().map(|dependency| {
        format!(
            "{} at {}:{}",
            dependency.kind.as_str(),
            dependency.span.line,
            dependency.span.col
        )
    }));
    let span = impact
        .dependent_statements
        .first()
        .map_or(op.id.span, |dependency| dependency.span);
    let message = format!(
        "removing `{id}` would orphan dependents: {}",
        dependents.join(", ")
    );
    dependency_failure("remove", loaded, "E303", span, message)
}

fn describe(path: &Path) -> Execution {
    let loaded = match load_scene("describe", path) {
        Ok(loaded) => loaded,
        Err(execution) => return *execution,
    };
    let objects = describe_objects(&loaded.file, resolved_scene(&loaded.scene), &loaded.source);
    let result = DescribeResult {
        command: "describe",
        path: path_string(path),
        objects,
        solve: loaded.scene.solve_summary().map(SolveReport::from),
    };
    success(CommandOutput::Describe(result), loaded.document())
}

fn inspect(path: &Path) -> Execution {
    let loaded = match load_scene("inspect", path) {
        Ok(loaded) => loaded,
        Err(execution) => return *execution,
    };
    let inspection = inspect_scene(&loaded.scene);
    let result = InspectResult {
        command: "inspect",
        path: path_string(path),
        human_diagnostics: inspection_diagnostics(path, &inspection.diagnostics),
        human_anchors: object_anchors(&loaded.file),
        inspection,
    };
    success(CommandOutput::Inspect(result), loaded.document())
}

fn check(paths: &[PathBuf], strict: bool) -> Execution {
    let mut files = Vec::new();
    let mut sources = Vec::new();
    let mut exit = Exit::Success;
    for path in paths {
        let checked = load_checked(path, strict);
        exit = dominant_exit(exit, checked.exit);
        files.push(checked.file);
        sources.extend(checked.sources);
    }
    check_result(files, sources, strict, exit)
}

fn load_checked(path: &Path, strict: bool) -> CheckedOutcome {
    let loaded = match load_scene("check", path) {
        Ok(loaded) => loaded,
        Err(execution) => return failed_check(path, *execution),
    };
    let file = match check_file(&loaded, strict) {
        Ok(file) => file,
        Err(execution) => return failed_check(path, *execution),
    };
    let exit = if file.passed {
        Exit::Success
    } else {
        Exit::CheckFailure
    };
    CheckedOutcome {
        file,
        sources: vec![loaded.document()],
        exit,
    }
}

fn check_file(loaded: &LoadedScene, strict: bool) -> Result<CheckFileResult, Box<Execution>> {
    let inspection = inspect_scene(&loaded.scene);
    let diagnostics = inspection_diagnostics(&loaded.path, &inspection.diagnostics);
    let assertions = evaluate_assertions(&loaded.file, resolved_scene(&loaded.scene))
        .map_err(|error| Box::new(eval_failure("check", &loaded.as_file(), error)))?;
    let passed = assertions.passed && (!strict || diagnostics.is_empty());
    Ok(CheckFileResult {
        path: path_string(&loaded.path),
        passed,
        solve: loaded.scene.solve_summary().map(SolveReport::from),
        diagnostics,
        assertions: Some(assertions),
    })
}

fn failed_check(path: &Path, execution: Execution) -> CheckedOutcome {
    let Execution {
        output,
        exit,
        sources,
    } = execution;
    let CommandOutput::Failure(failure) = output else {
        unreachable!("failed check must contain a structured failure")
    };
    CheckedOutcome {
        file: CheckFileResult {
            path: path_string(path),
            passed: false,
            solve: None,
            diagnostics: failure.diagnostics,
            assertions: None,
        },
        sources,
        exit,
    }
}

fn dominant_exit(current: Exit, candidate: Exit) -> Exit {
    if candidate.code() > current.code() {
        candidate
    } else {
        current
    }
}

fn check_result(
    files: Vec<CheckFileResult>,
    sources: Vec<SourceDocument>,
    strict: bool,
    exit: Exit,
) -> Execution {
    let passed = files.iter().all(|file| file.passed);
    Execution {
        output: CommandOutput::Check(CheckResult {
            command: "check",
            strict,
            passed,
            files,
        }),
        exit,
        sources,
    }
}

fn fmt(path: &Path) -> Execution {
    let loaded = match read_source("fmt", path) {
        Ok(loaded) => loaded,
        Err(execution) => return *execution,
    };
    let formatted = format(&loaded.source);
    if !formatted.diagnostics.is_empty() {
        return syntax_failure("fmt", &loaded, formatted.diagnostics);
    }
    let parsed = parse(&loaded.source);
    let result = FormatResult {
        source: formatted.source,
        ast: locus_syntax::to_interchange(&parsed.file),
    };
    success(CommandOutput::Format(result), loaded.document())
}

#[derive(Clone, Debug)]
struct LoadedSource {
    path: PathBuf,
    source: String,
}

impl LoadedSource {
    fn document(&self) -> SourceDocument {
        SourceDocument {
            path: path_string(&self.path),
            source: self.source.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct LoadedFile {
    path: PathBuf,
    source: String,
    file: File,
}

impl LoadedFile {
    fn document(&self) -> SourceDocument {
        SourceDocument {
            path: path_string(&self.path),
            source: self.source.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct LoadedScene {
    path: PathBuf,
    source: String,
    file: File,
    scene: Scene,
}

#[derive(Clone, Debug)]
struct CheckedOutcome {
    file: CheckFileResult,
    sources: Vec<SourceDocument>,
    exit: Exit,
}

impl LoadedScene {
    fn document(&self) -> SourceDocument {
        SourceDocument {
            path: path_string(&self.path),
            source: self.source.clone(),
        }
    }

    fn as_file(&self) -> LoadedFile {
        LoadedFile {
            path: self.path.clone(),
            source: self.source.clone(),
            file: self.file.clone(),
        }
    }
}

fn read_source(command: &'static str, path: &Path) -> Result<LoadedSource, Box<Execution>> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(LoadedSource {
            path: path.to_owned(),
            source,
        }),
        Err(error) => Err(Box::new(io_failure(command, path, &error))),
    }
}

fn load_parsed(command: &'static str, path: &Path) -> Result<LoadedFile, Box<Execution>> {
    let loaded = read_source(command, path)?;
    let parsed = parse(&loaded.source);
    if !parsed.diagnostics.is_empty() {
        return Err(Box::new(syntax_failure(
            command,
            &loaded,
            parsed.diagnostics,
        )));
    }
    Ok(LoadedFile {
        path: loaded.path,
        source: loaded.source,
        file: parsed.file,
    })
}

fn load_scene(command: &'static str, path: &Path) -> Result<LoadedScene, Box<Execution>> {
    let loaded = load_parsed(command, path)?;
    match Scene::new(loaded.file.clone()) {
        Ok(scene) => Ok(LoadedScene {
            path: loaded.path,
            source: loaded.source,
            file: loaded.file,
            scene,
        }),
        Err(error) => Err(Box::new(eval_failure(command, &loaded, error))),
    }
}

fn validate_and_write(
    command: &'static str,
    path: &Path,
    candidate: String,
    id: Option<&str>,
    action: &str,
) -> Execution {
    #[cfg(debug_assertions)]
    crate::rejected_requests::candidate(path, &candidate);
    let loaded = LoadedSource {
        path: path.to_owned(),
        source: candidate.clone(),
    };
    let parsed = parse(&candidate);
    if !parsed.diagnostics.is_empty() {
        return syntax_failure(command, &loaded, parsed.diagnostics);
    }
    if let Err(error) = Scene::new(parsed.file.clone()) {
        let file = LoadedFile {
            path: path.to_owned(),
            source: candidate.clone(),
            file: parsed.file,
        };
        return eval_failure(command, &file, error);
    }
    match fs::write(path, candidate) {
        Ok(()) => mutation(command, path, id, format!("{action} in {}", path.display())),
        Err(error) => io_failure(command, path, &error),
    }
}

fn mutation(command: &'static str, path: &Path, id: Option<&str>, message: String) -> Execution {
    Execution {
        output: CommandOutput::Mutation(MutationResult {
            command,
            path: path_string(path),
            changed: true,
            id: id.map(str::to_owned),
            message,
        }),
        exit: Exit::Success,
        sources: Vec::new(),
    }
}

fn success(output: CommandOutput, source: SourceDocument) -> Execution {
    Execution {
        output,
        exit: Exit::Success,
        sources: vec![source],
    }
}

fn syntax_failure(
    command: &'static str,
    loaded: &LoadedSource,
    diagnostics: Vec<Diagnostic>,
) -> Execution {
    let path = path_string(&loaded.path);
    let diagnostics = diagnostics
        .into_iter()
        .map(|diagnostic| syntax_diagnostic(&path, diagnostic))
        .collect();
    failure(command, diagnostics, Exit::Usage, loaded.document())
}

fn eval_failure(command: &'static str, loaded: &LoadedFile, error: EvalError) -> Execution {
    let mut notes = Vec::new();
    if let Some(object) = &error.object_id {
        notes.push(format!("while evaluating `{object}`"));
    }
    if let Some(measured) = error.measured_note() {
        notes.push(measured);
    }
    let diagnostic = CliDiagnostic {
        code: error.code.to_owned(),
        severity: Severity::Error,
        path: path_string(&loaded.path),
        span: error.span,
        message: error.message,
        notes,
        suggestion: None,
    };
    failure(
        command,
        vec![diagnostic],
        Exit::Evaluation,
        loaded.document(),
    )
}

fn dependency_failure(
    command: &'static str,
    loaded: &LoadedFile,
    code: &str,
    span: Span,
    message: String,
) -> Execution {
    let diagnostic = CliDiagnostic {
        code: code.to_owned(),
        severity: Severity::Error,
        path: path_string(&loaded.path),
        span,
        message,
        notes: Vec::new(),
        suggestion: None,
    };
    failure(
        command,
        vec![diagnostic],
        Exit::Evaluation,
        loaded.document(),
    )
}

fn edit_target_failure(command: &'static str, loaded: &LoadedFile, id: &str) -> Execution {
    dependency_failure(
        command,
        loaded,
        "E304",
        Span::empty(1, 1, 0),
        format!("no declaration has id `{id}`"),
    )
}

fn edit_usage_failure(command: &'static str, loaded: &LoadedFile, message: &str) -> Execution {
    let diagnostic = CliDiagnostic {
        code: "usage".to_owned(),
        severity: Severity::Error,
        path: path_string(&loaded.path),
        span: Span::empty(1, 1, 0),
        message: message.to_owned(),
        notes: Vec::new(),
        suggestion: Some(
            "pass exactly one --def expression/constructor or one --set key=value pair".to_owned(),
        ),
    };
    failure(command, vec![diagnostic], Exit::Usage, loaded.document())
}

fn io_failure(command: &'static str, path: &Path, error: &io::Error) -> Execution {
    let diagnostic = CliDiagnostic {
        code: "io".to_owned(),
        severity: Severity::Error,
        path: path_string(path),
        span: Span::empty(1, 1, 0),
        message: error.to_string(),
        notes: Vec::new(),
        suggestion: None,
    };
    Execution {
        output: CommandOutput::Failure(FailureResult {
            command,
            diagnostics: vec![diagnostic],
        }),
        exit: Exit::Usage,
        sources: Vec::new(),
    }
}

fn failure(
    command: &'static str,
    diagnostics: Vec<CliDiagnostic>,
    exit: Exit,
    source: SourceDocument,
) -> Execution {
    Execution {
        output: CommandOutput::Failure(FailureResult {
            command,
            diagnostics,
        }),
        exit,
        sources: vec![source],
    }
}

fn syntax_diagnostic(path: &str, diagnostic: Diagnostic) -> CliDiagnostic {
    CliDiagnostic {
        code: diagnostic.code.to_owned(),
        severity: Severity::Error,
        path: path.to_owned(),
        span: diagnostic.span,
        message: diagnostic.message,
        notes: diagnostic.notes,
        suggestion: diagnostic.suggestion,
    }
}

fn append_source(source: &str, appended: &str) -> String {
    let mut candidate = source.to_owned();
    if !candidate.is_empty() && !candidate.ends_with('\n') {
        candidate.push('\n');
    }
    candidate.push_str(appended);
    if !candidate.ends_with('\n') {
        candidate.push('\n');
    }
    candidate
}

fn find_op<'a>(file: &'a File, id: &str) -> Option<&'a OpStmt> {
    file.statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == id => Some(op),
            _ => None,
        })
}

fn replace_span(source: &str, span: Span, replacement: &str) -> String {
    let mut candidate = source.to_owned();
    candidate.replace_range(span.byte_start..span.byte_end, replacement);
    candidate
}

fn update_definition(source: &str, op: &OpStmt, definition: &str) -> Result<String, &'static str> {
    let definition = definition.trim();
    if definition.is_empty() {
        return Err("definition update requires one non-empty expression or constructor");
    }
    let candidate = replace_span(source, op.ctor.span(), definition);
    let parsed = parse(&candidate);
    if !parsed.diagnostics.is_empty() {
        return Err("definition update must be exactly one expression or constructor");
    }
    let updated = find_op(&parsed.file, &op.id.name)
        .ok_or("definition update must be exactly one expression or constructor")?;
    if candidate.get(updated.ctor.span().byte_start..updated.ctor.span().byte_end)
        != Some(definition)
    {
        return Err("definition update must not contain trailing source or comments");
    }
    Ok(candidate)
}

fn update_property(source: &str, op: &OpStmt, assignment: &str) -> Result<String, &'static str> {
    let Some((key, value)) = assignment.split_once('=') else {
        return Err("property update must use key=value syntax");
    };
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err("property update requires a non-empty key and value");
    }
    let candidate = if let Some(prop) = op.props.items.iter().find(|prop| prop.name.name == key) {
        replace_span(source, prop.value.span(), value)
    } else {
        insert_property(source, op, key, value)?
    };
    validate_property_candidate(&candidate, &op.id.name, key, value)?;
    Ok(candidate)
}

fn validate_property_candidate(
    candidate: &str,
    id: &str,
    key: &str,
    value: &str,
) -> Result<(), &'static str> {
    let parsed = parse(candidate);
    if !parsed.diagnostics.is_empty() {
        return Err("property update must be exactly one key/value pair");
    }
    let updated = find_op(&parsed.file, id).ok_or("property update lost its target declaration")?;
    let property = updated
        .props
        .items
        .iter()
        .find(|property| property.name.name == key)
        .ok_or("property update lost its target property")?;
    let parsed_value =
        candidate.get(property.value.span().byte_start..property.value.span().byte_end);
    if property.name.name != key || parsed_value != Some(value) {
        return Err("property update must not contain trailing source or properties");
    }
    Ok(())
}

fn insert_property(
    source: &str,
    op: &OpStmt,
    key: &str,
    value: &str,
) -> Result<String, &'static str> {
    let no_block = op.props.span.byte_start == op.props.span.byte_end;
    let insertion = if no_block {
        format!(" {{ {key}: {value} }}")
    } else if op.props.items.is_empty() {
        format!(" {key}: {value} ")
    } else {
        format!(", {key}: {value}")
    };
    let byte = if no_block {
        op.props.span.byte_start
    } else {
        op.props
            .span
            .byte_end
            .checked_sub(1)
            .ok_or("invalid property span")?
    };
    let mut candidate = source.to_owned();
    candidate.insert_str(byte, &insertion);
    Ok(candidate)
}

fn remove_physical_line(source: &str, span: Span) -> String {
    let start = source[..span.byte_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let end = source[span.byte_end..]
        .find('\n')
        .map_or(source.len(), |index| span.byte_end + index + 1);
    let mut candidate = source.to_owned();
    candidate.replace_range(start..end, "");
    candidate
}

fn describe_objects(file: &File, scene: &ResolvedScene, source: &str) -> Vec<ObjectReport> {
    file.statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) => scene
                .objects
                .iter()
                .find(|(id, _)| id.name == op.id.name)
                .map(|(_, object)| object_report(op, object, source)),
            _ => None,
        })
        .collect()
}

fn object_anchors(file: &File) -> Vec<ObjectAnchorReport> {
    file.statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) => Some(ObjectAnchorReport {
                id: op.id.name.clone(),
                span: op.id.span,
            }),
            _ => None,
        })
        .collect()
}

fn object_report(op: &OpStmt, object: &ResolvedObject, source: &str) -> ObjectReport {
    ObjectReport {
        id: op.id.name.clone(),
        kind: op.kind.as_str().to_owned(),
        span: op.span,
        definition: source[op.ctor.span().byte_start..op.ctor.span().byte_end].to_owned(),
        resolved: resolved_value(object),
    }
}

fn resolved_value(object: &ResolvedObject) -> ResolvedValue {
    match object {
        ResolvedObject::Num(_) | ResolvedObject::Angle(_) | ResolvedObject::Point(_) => {
            scalar_value(object)
        }
        ResolvedObject::Line(_) | ResolvedObject::Ray(_) | ResolvedObject::Segment(_) => {
            linear_value(object)
        }
        ResolvedObject::Circle(_)
        | ResolvedObject::Arc(_)
        | ResolvedObject::Ellipse(_)
        | ResolvedObject::Bspline(_) => curved_value(object),
        ResolvedObject::Polygon(_) | ResolvedObject::Rect(_) | ResolvedObject::Diamond(_) => {
            polygon_object_value(object)
        }
        ResolvedObject::Annotation(_) => ResolvedValue::Annotation,
    }
}

fn scalar_value(object: &ResolvedObject) -> ResolvedValue {
    match object {
        ResolvedObject::Num(number) => ResolvedValue::Number {
            value: number.value,
            dimension: dimension_name(number.dimension).to_owned(),
        },
        ResolvedObject::Angle(radians) => ResolvedValue::Angle { radians: *radians },
        ResolvedObject::Point(point) => ResolvedValue::Point {
            x: None,
            y: None,
            mark: point_mark_name(point.mark).to_owned(),
        },
        _ => unreachable!("scalar value received non-scalar object"),
    }
}

fn linear_value(object: &ResolvedObject) -> ResolvedValue {
    match object {
        ResolvedObject::Line(_) => ResolvedValue::Line {
            start: None,
            through: None,
        },
        ResolvedObject::Ray(_) => ResolvedValue::Ray {
            start: None,
            through: None,
        },
        ResolvedObject::Segment(segment) => ResolvedValue::Segment {
            start: None,
            end: None,
            length: locus_core::geom::measure::length(*segment),
        },
        _ => unreachable!("linear value received non-linear object"),
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "an allocated control-point count is exactly representable at every feasible size"
)]
fn curved_value(object: &ResolvedObject) -> ResolvedValue {
    match object {
        ResolvedObject::Circle(circle) => ResolvedValue::Circle {
            center: None,
            radius: circle.radius,
        },
        ResolvedObject::Arc(arc) => ResolvedValue::Arc {
            center: None,
            radius: arc.geometry.circle.radius,
            start: None,
            end: None,
            counterclockwise: arc.geometry.ccw,
            angle_mark_count: arc.angle_mark_count,
        },
        ResolvedObject::Ellipse(ellipse) => ResolvedValue::Ellipse {
            semimajor: ellipse.semimajor,
            semiminor: ellipse.semiminor,
            rotation: ellipse.rotation,
            area: core::f64::consts::PI * ellipse.semimajor * ellipse.semiminor,
        },
        ResolvedObject::Bspline(spline) => {
            let span_count = spline.controls().len() - 3;
            let mut knots = vec![0.0; 4];
            knots.extend((1..span_count).map(|index| index as f64 / span_count as f64));
            knots.extend([1.0; 4]);
            ResolvedValue::Bspline {
                degree: 3,
                knots,
                span_count,
            }
        }
        _ => unreachable!("curved value received non-curved object"),
    }
}

fn polygon_object_value(object: &ResolvedObject) -> ResolvedValue {
    match object {
        ResolvedObject::Polygon(polygon) => polygon_value(polygon, PolygonKind::Polygon),
        ResolvedObject::Rect(polygon) => polygon_value(polygon, PolygonKind::Rect),
        ResolvedObject::Diamond(polygon) => polygon_value(polygon, PolygonKind::Diamond),
        _ => unreachable!("polygon value received non-polygon object"),
    }
}

#[derive(Clone, Copy)]
enum PolygonKind {
    Polygon,
    Rect,
    Diamond,
}

fn polygon_value(polygon: &locus_core::Polygon, kind: PolygonKind) -> ResolvedValue {
    let vertices = None;
    let area = locus_core::geom::measure::area(polygon);
    let perimeter = locus_core::geom::measure::perimeter(polygon);
    match kind {
        PolygonKind::Polygon => ResolvedValue::Polygon {
            vertices,
            area,
            perimeter,
        },
        PolygonKind::Rect => ResolvedValue::Rect {
            vertices,
            area,
            perimeter,
        },
        PolygonKind::Diamond => ResolvedValue::Diamond {
            vertices,
            area,
            perimeter,
        },
    }
}

const fn dimension_name(dimension: Dimension) -> &'static str {
    match dimension {
        Dimension::Scalar => "scalar",
        Dimension::Length => "length",
        Dimension::Area => "area",
    }
}

const fn point_mark_name(mark: PointMark) -> &'static str {
    match mark {
        PointMark::Dot => "dot",
        PointMark::Cross => "cross",
        PointMark::None => "none",
    }
}

const fn resolved_scene(scene: &Scene) -> &ResolvedScene {
    match &scene.evaluation {
        Evaluation::Resolved(resolved)
        | Evaluation::Solved {
            scene: resolved, ..
        } => resolved,
    }
}

impl From<Point> for PointReport {
    fn from(point: Point) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

impl From<&SolveSummary> for SolveReport {
    fn from(summary: &SolveSummary) -> Self {
        Self {
            unknowns: summary.unknowns,
            equations: summary.equations,
            rank: summary.rank,
            attempts: summary.attempts,
            accepted_steps: summary.accepted_steps,
            max_normalized_residual: summary.max_normalized_residual,
            final_damping: summary.final_damping,
            dof: dof_report(summary),
            branch_resolutions: summary
                .branch_resolutions
                .iter()
                .map(|branch| BranchReport {
                    span: branch.span,
                    choice: branch.choice.clone(),
                })
                .collect(),
            hint_displacements: summary.hint_displacements.iter().map(hint_report).collect(),
            redundant_relations: summary
                .redundant_relations
                .iter()
                .map(relation_report)
                .collect(),
            relation_residuals: summary
                .relation_residuals
                .iter()
                .map(relation_report)
                .collect(),
        }
    }
}

fn dof_report(summary: &SolveSummary) -> DofReport {
    DofReport {
        global_remaining: summary.dof.global_remaining,
        objects: summary
            .dof
            .objects
            .iter()
            .map(|object| ObjectDofReport {
                id: object.id.clone(),
                span: object.span,
                hint_settled_dofs: object.hint_settled_dofs,
                suggestion: object.suggestion.clone(),
            })
            .collect(),
    }
}

fn hint_report(hint: &locus_core::HintDisplacement) -> HintReport {
    HintReport {
        id: hint.id.clone(),
        span: hint.span,
        source_components: hint.source_components.clone(),
        normalized_components: hint.normalized_components.clone(),
        max_normalized_component: hint.max_normalized_component,
        hint_far: hint.hint_far,
        hint_far_threshold: hint.hint_far_threshold,
    }
}

fn relation_report(relation: &locus_core::RelationReport) -> RelationReport {
    RelationReport {
        span: relation.span,
        max_normalized_residual: relation.max_normalized_residual,
        implicated_unknowns: relation.implicated_unknowns.clone(),
    }
}

fn inspection_diagnostics(path: &Path, diagnostics: &[InspectDiagnostic]) -> Vec<CliDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| inspect_diagnostic(path, diagnostic))
        .collect()
}

fn inspect_diagnostic(path: &Path, diagnostic: &InspectDiagnostic) -> CliDiagnostic {
    let (code, span, message) = inspect_diagnostic_parts(diagnostic);
    CliDiagnostic {
        code: code.to_owned(),
        severity: Severity::Warning,
        path: path_string(path),
        span,
        message,
        notes: Vec::new(),
        suggestion: None,
    }
}

fn inspect_diagnostic_parts(diagnostic: &InspectDiagnostic) -> (&'static str, Span, String) {
    match diagnostic {
        InspectDiagnostic::Degenerate { id, span, reason } => (
            "degenerate",
            *span,
            format!("`{id}` is degenerate: {reason}"),
        ),
        InspectDiagnostic::HintFar {
            id,
            span,
            actual,
            threshold,
        } => (
            "hint_far",
            *span,
            format!("`{id}` moved {actual:.6} from its hint (threshold {threshold:.6})"),
        ),
        InspectDiagnostic::LabelCollision { labels, spans } => (
            "label_collision",
            spans.first().copied().unwrap_or_default(),
            format!("labels collide: {}", labels.join(", ")),
        ),
        InspectDiagnostic::OutOfView { id, span, .. } => (
            "out_of_view",
            *span,
            format!("`{id}` lies outside the viewport"),
        ),
        InspectDiagnostic::Crowding {
            first,
            second,
            distance,
        } => (
            "crowding",
            Span::default(),
            format!("`{first}` and `{second}` are crowded at distance {distance:.6}"),
        ),
    }
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}
