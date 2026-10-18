function locationText(span) {
  if (!span) return '';
  return `Line ${span.line}, column ${span.col}: `;
}

const warningMessages = {
  degenerate: item => `${item.id}: ${item.reason}`,
  hint_far: item => `${item.id} moved away from its placement hint (${item.actual}; threshold ${item.threshold}).`,
  label_collision: item => `Labels need more space: ${item.labels.join(', ')}.`,
  out_of_view: item => `${item.id} is outside the viewport.`,
  crowding: item => `${item.first} and ${item.second} are close together (distance ${item.distance}).`,
};

function warningItem(item) {
  const message = warningMessages[item.kind]?.(item) || item.kind.replaceAll('_', ' ');
  return { span: item.span || item.spans?.[0], text: `Warning: ${message}` };
}

export function diagnosticItems(result) {
  const diagnostics = result.diagnostics.map(item => ({
    span: item.span,
    text: `${item.code}: ${item.message}`,
    details: [...item.notes, item.suggestion].filter(Boolean).join(' '),
  }));
  const warnings = (result.warnings || []).map(warningItem);
  const assertions = (result.assertions?.assertions || []).filter(item => !item.passed).map(item => ({
    span: item.span,
    text: `Assertion ${item.index + 1} failed.`,
    details: item.actuals.map(actual => `Column ${actual.span.col}: ${actual.value ?? actual.radians}`).join('\n'),
  }));
  return [...diagnostics, ...warnings, ...assertions];
}

export function showDiagnostics(list, result, selectSpan) {
  list.replaceChildren();
  for (const item of diagnosticItems(result)) {
    const row = document.createElement('li');
    const label = document.createElement(item.span ? 'button' : 'span');
    label.textContent = locationText(item.span) + item.text;
    if (item.span) {
      label.type = 'button';
      label.addEventListener('click', () => selectSpan(item.span));
    }
    row.append(label);
    if (item.details) {
      const details = document.createElement('p');
      details.textContent = item.details;
      row.append(details);
    }
    list.append(row);
  }
  list.hidden = !list.childElementCount;
}

export function resultSummary(result) {
  if (!result.ok) return 'Could not render. Check the diagnostics below.';
  const assertions = result.assertions.assertions;
  const passed = assertions.filter(item => item.passed).length;
  const warnings = result.warnings.length;
  return `Rendered · ${passed}/${assertions.length} assertions passed · ${warnings} warning(s).`;
}
