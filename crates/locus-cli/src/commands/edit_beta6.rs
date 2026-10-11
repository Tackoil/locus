//! Source edits for reusable calls, collections, and expanded member diagnostics.

use std::collections::BTreeSet;

use locus_core::DependencyGraph;
use locus_syntax::{
    CallStmt, CollectionKind, ExpandOutput, File, GroupStmt, Span, Statement, expand, parse,
};

use super::{
    Execution, LoadedFile, dependency_failure, edit_target_failure, edit_usage_failure,
    remove_physical_line, replace_span, validate_and_write,
};

pub(super) fn update(
    loaded: &LoadedFile,
    id: &str,
    definition: Option<&str>,
    property: Option<&str>,
) -> Option<Execution> {
    if let Some(failure) = qualified_failure("update", loaded, id) {
        return Some(failure);
    }
    let target = named_statement(&loaded.file, id)?;
    let candidate = match target {
        Statement::Call(call) => {
            update_call(&loaded.source, &loaded.file, call, definition, property)
        }
        Statement::Group(group) => {
            update_group(&loaded.source, &loaded.file, group, definition, property)
        }
        Statement::Fn(_) => Err(
            "function definitions must be edited in the source; --def updates calls and declarations",
        ),
        _ => return None,
    };
    Some(match candidate {
        Ok(candidate) => validate_and_write(
            "update",
            &loaded.path,
            candidate,
            Some(id),
            "updated declaration",
        ),
        Err(message) => edit_usage_failure("update", loaded, message),
    })
}

pub(super) fn remove(loaded: &LoadedFile, id: &str) -> Option<Execution> {
    if let Some(failure) = qualified_failure("remove", loaded, id) {
        return Some(failure);
    }
    if !has_reusable_source(&loaded.file) {
        return None;
    }
    let Some(target) = named_statement(&loaded.file, id) else {
        return Some(edit_target_failure("remove", loaded, id));
    };
    Some(remove_target(loaded, target, id))
}

fn remove_target(loaded: &LoadedFile, target: &Statement, id: &str) -> Execution {
    let expanded = expand(&loaded.file);
    if let Some(diagnostic) = expanded.diagnostics.first() {
        return dependency_failure(
            "remove",
            loaded,
            diagnostic.code,
            diagnostic.span,
            diagnostic.message.clone(),
        );
    }
    let dependents = match expanded_dependents(&expanded, id) {
        Ok(dependents) => dependents,
        Err(error) => {
            return dependency_failure("remove", loaded, error.code, error.span, error.message);
        }
    };
    if !dependents.is_empty() {
        return removal_failure(loaded, target.span(), id, &dependents.join(", "));
    }
    let candidate = remove_physical_line(&loaded.source, target.span());
    let parsed = parse(&candidate);
    let checked = expand(&parsed.file);
    if let Some(diagnostic) = checked.diagnostics.first() {
        return removal_failure(loaded, target.span(), id, &diagnostic.message);
    }
    validate_and_write(
        "remove",
        &loaded.path,
        candidate,
        Some(id),
        "removed declaration",
    )
}

fn removal_failure(loaded: &LoadedFile, span: Span, id: &str, dependents: &str) -> Execution {
    dependency_failure(
        "remove",
        loaded,
        "E303",
        span,
        format!("removing `{id}` would orphan dependents: {dependents}"),
    )
}

fn named_statement<'a>(file: &'a File, id: &str) -> Option<&'a Statement> {
    file.statements.iter().find(|statement| match statement {
        Statement::Op(op) => op.id.name == id,
        Statement::Call(call) => call.id.name == id,
        Statement::Group(group) => group.name.name == id,
        Statement::Fn(function) => function.name.name == id,
        _ => false,
    })
}

fn has_reusable_source(file: &File) -> bool {
    file.statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::Fn(_) | Statement::Group(_) | Statement::Call(_)
        )
    })
}

fn qualified_failure(command: &'static str, loaded: &LoadedFile, id: &str) -> Option<Execution> {
    if !id.contains('.') {
        return None;
    }
    let expanded = expand(&loaded.file);
    let origin = expanded
        .file
        .statements
        .iter()
        .enumerate()
        .find_map(|(index, statement)| {
            matches!(statement, Statement::Op(op) if op.id.name == id)
                .then(|| expanded.origins.get(index))
                .flatten()
        })
        .or_else(|| {
            expanded
                .collections
                .get(id)
                .map(|collection| &collection.origin)
        })?;
    let calls = origin
        .invocations
        .iter()
        .map(|call| {
            format!(
                "`{}` as `{}` at {}:{}",
                call.function, call.instance, call.span.line, call.span.col
            )
        })
        .collect::<Vec<_>>()
        .join(" → ");
    Some(dependency_failure(
        command,
        loaded,
        "E305",
        origin.span,
        format!(
            "qualified member `{id}` is not a top-level edit target; edit its definition at body line {} or the call {calls}",
            origin.span.line
        ),
    ))
}

fn update_call(
    source: &str,
    file: &File,
    call: &CallStmt,
    definition: Option<&str>,
    property: Option<&str>,
) -> Result<String, &'static str> {
    if property.is_some() {
        return Err(
            "instance calls accept --def function(arguments); edit their optional class in the source",
        );
    }
    let replacement = named_fragment(definition, &call.id.name, None)?;
    let candidate = replace_span(source, invocation_span(source, call), &replacement);
    let parsed = parse(&candidate);
    if !parsed.diagnostics.is_empty() || parsed.file.statements.len() != file.statements.len() {
        return Err("instance update must be exactly one function(arguments) call");
    }
    let Some(Statement::Call(updated)) = named_statement(&parsed.file, &call.id.name) else {
        return Err("instance update must be exactly one function(arguments) call");
    };
    let span = invocation_span(&candidate, updated);
    if candidate.get(span.byte_start..span.byte_end) != Some(replacement.as_str()) {
        return Err("instance update must not contain trailing source, comments, or a class");
    }
    Ok(candidate)
}

fn invocation_span(source: &str, call: &CallStmt) -> Span {
    let mut span = call.span;
    if let Some(attr) = &call.attr {
        span.byte_end = attr.span.byte_start;
    }
    span.byte_end = span.byte_start + source[span.byte_start..span.byte_end].trim_end().len();
    span
}

fn update_group(
    source: &str,
    file: &File,
    group: &GroupStmt,
    definition: Option<&str>,
    property: Option<&str>,
) -> Result<String, &'static str> {
    if property.is_some() {
        return Err("groups accept --def group(member, ...); groups do not have properties");
    }
    let replacement = named_fragment(definition, &group.name.name, Some("group"))?;
    let candidate = replace_span(source, group.span, &replacement);
    let parsed = parse(&candidate);
    if !parsed.diagnostics.is_empty() || parsed.file.statements.len() != file.statements.len() {
        return Err("group update must be exactly one group(member, ...) expression");
    }
    let Some(Statement::Group(updated)) = named_statement(&parsed.file, &group.name.name) else {
        return Err("group update must be exactly one group(member, ...) expression");
    };
    if candidate.get(updated.span.byte_start..updated.span.byte_end) != Some(replacement.as_str()) {
        return Err("group update must not contain trailing source or comments");
    }
    Ok(candidate)
}

fn named_fragment(
    definition: Option<&str>,
    id: &str,
    expected: Option<&str>,
) -> Result<String, &'static str> {
    let definition = definition.unwrap_or_default().trim();
    let (name, arguments) = definition
        .split_once('(')
        .ok_or("definition update requires a function(arguments) or group(members) expression")?;
    let name = name.trim();
    if name.is_empty() || expected.is_some_and(|expected| name != expected) {
        return Err("definition update has an incompatible declaration kind");
    }
    Ok(format!("{name} {id}({arguments}"))
}

fn expanded_dependents(
    expanded: &ExpandOutput,
    id: &str,
) -> Result<Vec<String>, locus_core::DependencyError> {
    let owned = expanded
        .file
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) if owns(id, &op.id.name) => Some(op.id.name.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let file = dependency_file(expanded, id, &owned);
    let graph = DependencyGraph::extract_with_collections(&file, &expanded.collections)?;
    let mut dependents = BTreeSet::new();
    let mut targets = owned.clone();
    targets.insert(id.to_owned());
    let mut removed = targets.clone();
    for member in &targets {
        let impact = graph.removal_impact(member);
        removed.extend(impact.orphaned_declarations.iter().cloned());
        dependents.extend(
            impact
                .orphaned_declarations
                .into_iter()
                .filter(|orphan| !owns(id, orphan)),
        );
        dependents.extend(impact.dependent_statements.iter().map(|statement| {
            format!(
                "{} at {}:{}",
                statement.kind.as_str(),
                statement.span.line,
                statement.span.col
            )
        }));
    }
    collection_dependents(expanded, id, &removed, &mut dependents);
    Ok(dependents.into_iter().collect())
}

fn dependency_file(expanded: &ExpandOutput, id: &str, owned: &BTreeSet<String>) -> File {
    let statements = expanded
        .file
        .statements
        .iter()
        .zip(&expanded.origins)
        .filter_map(|(statement, origin)| {
            if let Statement::Op(op) = statement {
                let mut op = op.clone();
                if owned.contains(&op.id.name) {
                    op.where_clauses.clear();
                }
                return Some(Statement::Op(op));
            }
            if origin
                .invocations
                .iter()
                .any(|call| owns(id, &call.instance))
            {
                return None;
            }
            Some(statement.clone())
        })
        .collect();
    File {
        statements,
        span: expanded.file.span,
    }
}

fn collection_dependents(
    expanded: &ExpandOutput,
    id: &str,
    removed: &BTreeSet<String>,
    dependents: &mut BTreeSet<String>,
) {
    for collection in expanded.collections.values() {
        if collection.kind == CollectionKind::Group
            && !owns(id, &collection.id)
            && collection
                .members
                .iter()
                .any(|member| removed.contains(member))
        {
            dependents.insert(format!(
                "group `{}` at {}:{}",
                collection.id, collection.span.line, collection.span.col
            ));
        }
    }
}

fn owns(id: &str, member: &str) -> bool {
    member == id
        || member
            .strip_prefix(id)
            .is_some_and(|suffix| suffix.starts_with('.'))
}
