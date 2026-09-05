//! Stable serialized command results and their human presentation.
#![allow(
    missing_docs,
    reason = "output fields are the self-describing CLI schema"
)]

use std::fmt::{self, Write as _};
use std::io::{self, Write as _};

use locus_core::{ActualValue, AssertionReport, Incidence, InspectionReport};
use locus_syntax::{Interchange, Span};
use miette::{LabeledSpan, MietteDiagnostic, NamedSource, Report, Severity as MietteSeverity};
use serde::{Serialize, Serializer};

use crate::exit::Exit;

#[derive(Clone, Debug)]
pub(crate) struct SourceDocument {
    pub path: String,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CliDiagnostic {
    pub code: String,
    pub severity: Severity,
    pub path: String,
    pub span: Span,
    pub message: String,
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MutationResult {
    pub command: &'static str,
    pub path: String,
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FailureResult {
    pub command: &'static str,
    pub diagnostics: Vec<CliDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DescribeResult {
    pub command: &'static str,
    pub path: String,
    pub objects: Vec<ObjectReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solve: Option<SolveReport>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ObjectReport {
    pub id: String,
    pub kind: String,
    pub span: Span,
    pub definition: String,
    pub resolved: ResolvedValue,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ResolvedValue {
    Number {
        value: f64,
        dimension: String,
    },
    Angle {
        radians: f64,
    },
    Point {
        x: f64,
        y: f64,
        mark: String,
    },
    Line {
        start: PointReport,
        through: PointReport,
    },
    Ray {
        start: PointReport,
        through: PointReport,
    },
    Segment {
        start: PointReport,
        end: PointReport,
    },
    Circle {
        center: PointReport,
        radius: f64,
    },
    Arc {
        center: PointReport,
        radius: f64,
        start: PointReport,
        end: PointReport,
        counterclockwise: bool,
        angle_mark_count: Option<u8>,
    },
    Polygon {
        vertices: Vec<PointReport>,
    },
    Rect {
        vertices: Vec<PointReport>,
    },
    Diamond {
        vertices: Vec<PointReport>,
    },
    Annotation {
        value: AnnotationReport,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct PointReport {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum AnnotationReport {
    Label {
        anchor: AnchorReport,
        text: locus_syntax::TextValue,
        direction: String,
        gap: Option<f64>,
    },
    Arrow {
        from: AnchorReport,
        to: AnchorReport,
        head: String,
        bend: f64,
    },
    Tick {
        start: PointReport,
        end: PointReport,
        count: u8,
    },
    RightAngle {
        vertex: PointReport,
        first: PointReport,
        second: PointReport,
        size: Option<f64>,
    },
    TextBlock {
        text: locus_syntax::TextValue,
        placement: TextBlockPlacementReport,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TextBlockPlacementReport {
    Below,
    Above,
    Left,
    Right,
    Anchor {
        anchor: AnchorReport,
        direction: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum AnchorReport {
    Point { point: PointReport },
    AnnotationCenter { id: String, span: Span },
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SolveReport {
    pub unknowns: usize,
    pub equations: usize,
    pub rank: usize,
    pub attempts: usize,
    pub accepted_steps: usize,
    pub max_normalized_residual: f64,
    pub final_damping: f64,
    pub dof: DofReport,
    pub branch_resolutions: Vec<BranchReport>,
    pub hint_displacements: Vec<HintReport>,
    pub redundant_relations: Vec<RelationReport>,
    pub relation_residuals: Vec<RelationReport>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DofReport {
    pub global_remaining: usize,
    pub objects: Vec<ObjectDofReport>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ObjectDofReport {
    pub id: String,
    pub span: Span,
    pub hint_settled_dofs: usize,
    pub suggestion: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BranchReport {
    pub span: Span,
    pub choice: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct HintReport {
    pub id: String,
    pub span: Span,
    pub source_components: Vec<f64>,
    pub normalized_components: Vec<f64>,
    pub max_normalized_component: f64,
    pub hint_far: bool,
    pub hint_far_threshold: f64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RelationReport {
    pub span: Span,
    pub max_normalized_residual: f64,
    pub implicated_unknowns: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct InspectResult {
    pub command: &'static str,
    pub path: String,
    #[serde(flatten)]
    pub inspection: InspectionReport,
    #[serde(skip)]
    pub human_diagnostics: Vec<CliDiagnostic>,
    #[serde(skip)]
    pub human_anchors: Vec<ObjectAnchorReport>,
}

#[derive(Clone, Debug)]
pub(crate) struct ObjectAnchorReport {
    pub id: String,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CheckResult {
    pub command: &'static str,
    pub strict: bool,
    pub passed: bool,
    pub files: Vec<CheckFileResult>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CheckFileResult {
    pub path: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solve: Option<SolveReport>,
    pub diagnostics: Vec<CliDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions: Option<AssertionReport>,
}

#[derive(Clone, Debug)]
pub(crate) struct FormatResult {
    pub source: String,
    pub ast: Interchange,
}

impl Serialize for FormatResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.ast.serialize(serializer)
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RenderResult {
    pub command: &'static str,
    pub path: String,
    pub implemented: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum CommandOutput {
    Mutation(MutationResult),
    Describe(DescribeResult),
    Inspect(InspectResult),
    Check(CheckResult),
    Format(FormatResult),
    Failure(FailureResult),
    Render(RenderResult),
}

#[derive(Clone, Debug)]
pub(crate) struct Execution {
    pub output: CommandOutput,
    pub exit: Exit,
    pub sources: Vec<SourceDocument>,
}

pub(crate) fn emit(execution: &Execution, json: bool) {
    if json {
        emit_json(&execution.output);
    } else {
        emit_human(execution);
    }
}

pub(crate) fn usage_error(message: &str) -> Execution {
    let diagnostic = CliDiagnostic {
        code: "usage".to_owned(),
        severity: Severity::Error,
        path: "<arguments>".to_owned(),
        span: Span::default(),
        message: message.to_owned(),
        notes: Vec::new(),
        suggestion: None,
    };
    Execution {
        output: CommandOutput::Failure(FailureResult {
            command: "usage",
            diagnostics: vec![diagnostic],
        }),
        exit: Exit::Usage,
        sources: Vec::new(),
    }
}

fn emit_json(output: &CommandOutput) {
    let result = serde_json::to_writer_pretty(io::stdout(), output);
    if result.is_ok() {
        let _ = writeln!(io::stdout());
    }
}

fn emit_human(execution: &Execution) {
    match &execution.output {
        CommandOutput::Mutation(value) => println!("{}", value.message),
        CommandOutput::Describe(value) => println!("{}", human_describe(value)),
        CommandOutput::Inspect(value) => {
            println!("{}", human_inspect(value));
            emit_diagnostics(&value.human_diagnostics, &execution.sources);
        }
        CommandOutput::Check(value) => {
            println!("{}", human_check(value));
            emit_check_diagnostics(value, &execution.sources);
        }
        CommandOutput::Format(value) => print!("{}", value.source),
        CommandOutput::Failure(value) => emit_diagnostics(&value.diagnostics, &execution.sources),
        CommandOutput::Render(value) => eprintln!("error: {}", value.message),
    }
}

fn human_describe(result: &DescribeResult) -> String {
    let mut output = format!("{} object(s) in {}", result.objects.len(), result.path);
    for object in &result.objects {
        let _ = write!(
            output,
            "\n  {} {} @{}:{} = {} -> {}",
            object.kind,
            object.id,
            object.span.line,
            object.span.col,
            object.definition,
            resolved_summary(&object.resolved)
        );
    }
    if let Some(solve) = &result.solve {
        let _ = write!(
            output,
            "\nsolve: {} unknown(s), {} equation(s), rank {}, {} DOF remaining, residual {:.6e}",
            solve.unknowns,
            solve.equations,
            solve.rank,
            solve.dof.global_remaining,
            solve.max_normalized_residual
        );
        for object in &solve.dof.objects {
            let _ = write!(
                output,
                "\n  DOF {}: {} hint-settled ({})",
                object.id, object.hint_settled_dofs, object.suggestion
            );
        }
        for hint in &solve.hint_displacements {
            let _ = write!(
                output,
                "\n  hint {}: max displacement {:.6e}{}",
                hint.id,
                hint.max_normalized_component,
                if hint.hint_far { " (far)" } else { "" }
            );
        }
        for branch in &solve.branch_resolutions {
            let _ = write!(
                output,
                "\n  branch at {}:{}: {}",
                branch.span.line, branch.span.col, branch.choice
            );
        }
    }
    output
}

fn resolved_summary(value: &ResolvedValue) -> String {
    match value {
        ResolvedValue::Number { .. }
        | ResolvedValue::Angle { .. }
        | ResolvedValue::Point { .. } => scalar_summary(value),
        ResolvedValue::Line { .. } | ResolvedValue::Ray { .. } | ResolvedValue::Segment { .. } => {
            linear_summary(value)
        }
        ResolvedValue::Circle { .. } | ResolvedValue::Arc { .. } => curved_summary(value),
        ResolvedValue::Polygon { .. }
        | ResolvedValue::Rect { .. }
        | ResolvedValue::Diamond { .. } => polygon_object_summary(value),
        ResolvedValue::Annotation { value } => annotation_summary(value),
    }
}

fn scalar_summary(value: &ResolvedValue) -> String {
    match value {
        ResolvedValue::Point { x, y, mark } => format!("({x:.6}, {y:.6}), mark {mark}"),
        ResolvedValue::Number { value, dimension } => format!("{value:.6} {dimension}"),
        ResolvedValue::Angle { radians } => format!("{radians:.6} rad"),
        _ => unreachable!("scalar summary received non-scalar value"),
    }
}

fn linear_summary(value: &ResolvedValue) -> String {
    let (first_label, first, second_label, second) = match value {
        ResolvedValue::Line { start, through } | ResolvedValue::Ray { start, through } => {
            ("start", start, "through", through)
        }
        ResolvedValue::Segment { start, end } => ("start", start, "end", end),
        _ => unreachable!("linear summary received non-linear value"),
    };
    format!(
        "{first_label} {}, {second_label} {}",
        point_summary(first),
        point_summary(second)
    )
}

fn curved_summary(value: &ResolvedValue) -> String {
    match value {
        ResolvedValue::Circle { center, radius } => {
            format!("center {}, radius {radius:.6}", point_summary(center))
        }
        ResolvedValue::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
            angle_mark_count,
        } => format!(
            "center {}, radius {radius:.6}, start {}, end {}, counterclockwise {counterclockwise}, angle marks {}",
            point_summary(center),
            point_summary(start),
            point_summary(end),
            angle_mark_count.map_or_else(|| "none".to_owned(), |count| count.to_string())
        ),
        _ => unreachable!("curved summary received non-curved value"),
    }
}

fn polygon_object_summary(value: &ResolvedValue) -> String {
    let (ResolvedValue::Polygon { vertices }
    | ResolvedValue::Rect { vertices }
    | ResolvedValue::Diamond { vertices }) = value
    else {
        unreachable!("polygon summary received non-polygon value")
    };
    polygon_summary("vertices", vertices)
}

fn point_summary(point: &PointReport) -> String {
    format!("({:.6}, {:.6})", point.x, point.y)
}

fn polygon_summary(label: &str, vertices: &[PointReport]) -> String {
    let points = vertices
        .iter()
        .map(point_summary)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{label} [{points}]")
}

fn annotation_summary(annotation: &AnnotationReport) -> String {
    match annotation {
        AnnotationReport::Label {
            anchor,
            text,
            direction,
            gap,
        } => format!(
            "label anchor {}, text={}, direction {direction}, gap {}",
            anchor_summary(anchor),
            text_summary(text),
            gap.map_or_else(|| "default".to_owned(), |value| format!("{value:.6}"))
        ),
        AnnotationReport::Arrow {
            from,
            to,
            head,
            bend,
        } => format!(
            "arrow from {}, to {}, head {head}, bend {bend:.6}",
            anchor_summary(from),
            anchor_summary(to)
        ),
        AnnotationReport::Tick { start, end, count } => format!(
            "tick start {}, end {}, count {count}",
            point_summary(start),
            point_summary(end)
        ),
        AnnotationReport::RightAngle {
            vertex,
            first,
            second,
            size,
        } => format!(
            "right angle vertex {}, first {}, second {}, size {}",
            point_summary(vertex),
            point_summary(first),
            point_summary(second),
            size.map_or_else(|| "default".to_owned(), |value| format!("{value:.6}"))
        ),
        AnnotationReport::TextBlock { text, placement } => {
            format!(
                "text block text={}, {}",
                text_summary(text),
                placement_summary(placement)
            )
        }
    }
}

fn anchor_summary(anchor: &AnchorReport) -> String {
    match anchor {
        AnchorReport::Point { point } => point_summary(point),
        AnchorReport::AnnotationCenter { id, span } => {
            format!("center of {id}@{}:{}", span.line, span.col)
        }
    }
}

fn placement_summary(placement: &TextBlockPlacementReport) -> String {
    match placement {
        TextBlockPlacementReport::Below => "placement below".to_owned(),
        TextBlockPlacementReport::Above => "placement above".to_owned(),
        TextBlockPlacementReport::Left => "placement left".to_owned(),
        TextBlockPlacementReport::Right => "placement right".to_owned(),
        TextBlockPlacementReport::Anchor { anchor, direction } => {
            format!("anchor {}, direction {direction}", anchor_summary(anchor))
        }
    }
}

fn text_summary(text: &locus_syntax::TextValue) -> String {
    let source = text
        .segments
        .iter()
        .map(|segment| segment.source.as_str())
        .collect::<String>();
    format!("{source:?}")
}

fn human_inspect(result: &InspectResult) -> String {
    let mut output = format!(
        "inspection for {}: {} distance(s), {} triangle angle(s), {} line ratio(s), {} incidence(s), {} diagnostic(s)",
        result.path,
        result.inspection.measurements.distances.len(),
        result.inspection.measurements.triangle_angles.len(),
        result.inspection.measurements.line_ratios.len(),
        result.inspection.incidences.len(),
        result.inspection.diagnostics.len()
    );
    write_distance_measurements(&mut output, result);
    write_triangle_angles(&mut output, result);
    write_line_ratios(&mut output, result);
    write_incidences(&mut output, result);
    output
}

fn write_distance_measurements(output: &mut String, result: &InspectResult) {
    for measurement in &result.inspection.measurements.distances {
        let _ = write!(
            output,
            "\n  distance {}--{} = {:.6}",
            source_anchor(result, &measurement.first),
            source_anchor(result, &measurement.second),
            measurement.distance
        );
    }
}

fn write_triangle_angles(output: &mut String, result: &InspectResult) {
    for measurement in &result.inspection.measurements.triangle_angles {
        let _ = write!(
            output,
            "\n  angle {}--{}--{} = {:.6} rad",
            source_anchor(result, &measurement.first),
            source_anchor(result, &measurement.vertex),
            source_anchor(result, &measurement.second),
            measurement.radians
        );
    }
}

fn write_line_ratios(output: &mut String, result: &InspectResult) {
    for measurement in &result.inspection.measurements.line_ratios {
        let integer = measurement.integer_ratio.map_or_else(String::new, |ratio| {
            format!(" ({}:{})", ratio.left, ratio.right)
        });
        let _ = write!(
            output,
            "\n  ratio {}--{}--{} = {:.6}{integer}",
            source_anchor(result, &measurement.start),
            source_anchor(result, &measurement.point),
            source_anchor(result, &measurement.end),
            measurement.ratio
        );
    }
}

fn write_incidences(output: &mut String, result: &InspectResult) {
    for incidence in &result.inspection.incidences {
        let _ = write!(
            output,
            "\n  incidence {}",
            incidence_summary(result, incidence)
        );
    }
}

fn incidence_summary(result: &InspectResult, incidence: &Incidence) -> String {
    match incidence {
        Incidence::PointOnObject { point, object } => format!(
            "{} on {}",
            source_anchor(result, point),
            source_anchor(result, object)
        ),
        Incidence::Tangent { first, second } => format!(
            "{} tangent to {}",
            source_anchor(result, first),
            source_anchor(result, second)
        ),
        Incidence::RatioAlongLine {
            start,
            point,
            end,
            left,
            right,
        } => format!(
            "{} on {}--{} with {}{}:{}{} = {left}:{right}",
            source_anchor(result, point),
            source_anchor(result, start),
            source_anchor(result, end),
            start,
            point,
            point,
            end
        ),
    }
}

fn source_anchor(result: &InspectResult, id: &str) -> String {
    result
        .human_anchors
        .iter()
        .find(|anchor| anchor.id == id)
        .map_or_else(
            || id.to_owned(),
            |anchor| format!("{id}@{}:{}", anchor.span.line, anchor.span.col),
        )
}

fn human_check(result: &CheckResult) -> String {
    let state = if result.passed { "passed" } else { "failed" };
    let assertions = result
        .files
        .iter()
        .filter_map(|file| file.assertions.as_ref())
        .map(|report| report.assertions.len())
        .sum::<usize>();
    let mut output = format!(
        "check {state}: {} file(s), {assertions} assertion(s){}",
        result.files.len(),
        if result.strict { ", strict" } else { "" }
    );
    for file in &result.files {
        let assertion_count = file
            .assertions
            .as_ref()
            .map_or(0, |report| report.assertions.len());
        let state = if file.passed { "passed" } else { "failed" };
        let _ = write!(
            output,
            "\n  {state} {}: {assertion_count} assertion(s), {} diagnostic(s)",
            file.path,
            file.diagnostics.len()
        );
    }
    output
}

fn emit_diagnostics(diagnostics: &[CliDiagnostic], sources: &[SourceDocument]) {
    for diagnostic in diagnostics {
        let source = sources.iter().find(|source| source.path == diagnostic.path);
        if let Some(source) = source {
            eprintln!("{:?}", miette_report(diagnostic, source));
        } else {
            eprintln!(
                "{}: {}[{}]: {}",
                diagnostic.path, diagnostic.severity, diagnostic.code, diagnostic.message
            );
        }
    }
}

fn emit_check_diagnostics(result: &CheckResult, sources: &[SourceDocument]) {
    for file in &result.files {
        emit_diagnostics(&file.diagnostics, sources);
        let assertions = assertion_diagnostics(file);
        emit_diagnostics(&assertions, sources);
    }
}

fn assertion_diagnostics(file: &CheckFileResult) -> Vec<CliDiagnostic> {
    file.assertions
        .iter()
        .flat_map(|report| &report.assertions)
        .filter(|assertion| !assertion.passed)
        .map(|assertion| CliDiagnostic {
            code: "assert_failed".to_owned(),
            severity: Severity::Error,
            path: file.path.clone(),
            span: assertion.span,
            message: format!("assertion {} failed", assertion.index + 1),
            notes: assertion
                .actuals
                .iter()
                .map(|actual| {
                    format!(
                        "{}:{} = {}",
                        actual.span.line,
                        actual.span.col,
                        actual_value(&actual.actual)
                    )
                })
                .collect(),
            suggestion: Some(
                "inspect the measured subterms and their upstream definitions".to_owned(),
            ),
        })
        .collect()
}

fn actual_value(actual: &ActualValue) -> String {
    match actual {
        ActualValue::Number { value, dimension } => {
            format!("{value:.12} {dimension:?}").to_ascii_lowercase()
        }
        ActualValue::Angle { radians } => format!("{radians:.12} rad"),
        ActualValue::Boolean { value } => value.to_string(),
    }
}

fn miette_report(diagnostic: &CliDiagnostic, source: &SourceDocument) -> Report {
    let length = diagnostic
        .span
        .byte_end
        .saturating_sub(diagnostic.span.byte_start);
    let label = LabeledSpan::new_with_span(
        Some(diagnostic.message.clone()),
        (diagnostic.span.byte_start, length),
    );
    let mut rendered = MietteDiagnostic::new(diagnostic.message.clone())
        .with_code(diagnostic.code.clone())
        .with_severity(match diagnostic.severity {
            Severity::Error => MietteSeverity::Error,
            Severity::Warning => MietteSeverity::Warning,
        })
        .with_label(label);
    let mut help = diagnostic.notes.clone();
    if let Some(suggestion) = &diagnostic.suggestion {
        help.push(suggestion.clone());
    }
    if !help.is_empty() {
        rendered = rendered.with_help(help.join("\n"));
    }
    Report::new(rendered).with_source_code(NamedSource::new(&source.path, source.source.clone()))
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}
