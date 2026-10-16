"use strict";
const dump = JSON.parse(document.getElementById("dump").textContent);
const byId = id => document.getElementById(id);
const entities = new Map(dump.entities.map(entity => [entity.id, entity]));
let position = Math.max(0, dump.steps.reduce((last, step, index) => step.svg ? index : last, -1));
let visible = [];
const measuredLabels = new Map((dump.steps.find(step => step.name === "measure_text")?.data?.measurements || [])
  .filter(measured => measured.outline?.svg).map(measured => [measured.id, measured.outline.svg]));
const glyphs = new Map();
let shapes = new Map();
let groupShapes = new Map();
const label = name => name.replaceAll("_", " ");

function members(id, seen = new Set()) {
  if (seen.has(id)) return seen;
  seen.add(id);
  for (const member of entities.get(id)?.members || []) members(member, seen);
  return seen;
}

function sourceDescription(id) {
  const entity = entities.get(id);
  if (!entity) return id;
  const groups = dump.entities.filter(item => item.members && members(item.id).has(id) && item.id !== id);
  const location = entity.span ? ` — ${dump.path}:${entity.span.line}:${entity.span.col}` : "";
  const ownership = groups.length ? `\nGroups: ${groups.map(group => group.id).join(", ")}` : "";
  const name = entity.source_name && entity.source_name !== id ? ` (source: ${entity.source_name})` : "";
  return `${id}${name}${location}${ownership}`;
}

function showSource(id) {
  showSourceLine(entities.get(id)?.span?.line);
}

function showSourceLine(line) {
  document.querySelectorAll(".source-line.active").forEach(line => line.classList.remove("active"));
  byId(`line-${line}`)?.classList.add("active");
}

function hover(id, event) {
  const text = sourceDescription(id);
  byId("hover").textContent = text;
  const tooltip = byId("tooltip");
  tooltip.textContent = text;
  tooltip.hidden = false;
  tooltip.style.left = `${Math.max(8, Math.min(event.clientX + 15, window.innerWidth - 375))}px`;
  tooltip.style.top = `${Math.min(event.clientY + 15, window.innerHeight - 100)}px`;
  showSource(id);
  highlight([id]);
  showRelations(relatedConstraints(id));
}

function bindShape(element, id) {
  element.addEventListener("pointermove", event => hover(id, event));
  element.addEventListener("pointerleave", () => {
    byId("tooltip").hidden = true;
    highlight();
    showSourceLine();
  });
  element.addEventListener("click", event => {
    byId(event.shiftKey ? "second" : "first").value = id;
    updateRelations();
    hover(id, event);
  });
}

// Dump files are untrusted. Rebuild an SVG from a small drawing-only allowlist.
function safeSvg(source) {
  const parsed = new DOMParser().parseFromString(source, "image/svg+xml");
  if (parsed.querySelector("parsererror") || parsed.documentElement.localName !== "svg") return null;
  const tags = new Set(["svg", "g", "path", "circle", "ellipse", "rect", "line", "polyline", "polygon", "title", "text"]);
  const attrs = new Set(["id", "viewBox", "width", "height", "d", "cx", "cy", "r", "rx", "ry", "x", "y", "x1", "x2", "y1", "y2", "points", "fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin", "stroke-dasharray", "opacity", "fill-opacity", "stroke-opacity", "fill-rule", "transform", "data-source-start", "data-source-end", "data-measured-label", "data-measurement-gallery", "font-size"]);
  function clean(node) {
    if (node.nodeType === Node.TEXT_NODE) return document.createTextNode(node.textContent);
    if (node.nodeType !== Node.ELEMENT_NODE || !tags.has(node.localName)) return null;
    const copy = document.createElementNS("http://www.w3.org/2000/svg", node.localName);
    for (const attribute of node.attributes) {
      if (attrs.has(attribute.name) && !/url\s*\(|[<>]/i.test(attribute.value)) copy.setAttribute(attribute.name, attribute.value);
    }
    for (const child of node.childNodes) {
      const safe = clean(child);
      if (safe) copy.append(safe);
    }
    return copy;
  }
  return clean(parsed.documentElement);
}

function expandMeasuredLabels(svg) {
  for (const placeholder of svg.querySelectorAll("[data-measured-label]")) {
    const id = placeholder.getAttribute("data-measured-label");
    if (!glyphs.has(id)) {
      const source = measuredLabels.get(id);
      glyphs.set(id, source ? safeSvg(source) : null);
    }
    const outline = glyphs.get(id);
    if (outline) placeholder.append(...[...outline.childNodes].map(node => node.cloneNode(true)));
  }
}

function groupBounds(id) {
  const boxes = [...members(id)].map(member => shapes.get(member)).filter(Boolean).map(element => element.getBBox());
  if (!boxes.length) return null;
  const x = Math.min(...boxes.map(box => box.x));
  const y = Math.min(...boxes.map(box => box.y));
  const right = Math.max(...boxes.map(box => box.x + box.width));
  const bottom = Math.max(...boxes.map(box => box.y + box.height));
  return {x, y, width: right - x, height: bottom - y};
}

function drawGroup(svg, entity) {
  const box = groupBounds(entity.id);
  if (!box) return;
  const overlay = document.createElementNS(svg.namespaceURI, "g");
  overlay.classList.add("group-overlay");
  for (const name of ["group-hit-area", "group-box"]) {
    const rect = document.createElementNS(svg.namespaceURI, "rect");
    for (const [key, value] of Object.entries(box)) rect.setAttribute(key, value);
    rect.classList.add(name);
    overlay.append(rect);
  }
  const text = document.createElementNS(svg.namespaceURI, "text");
  text.setAttribute("x", box.x + 3);
  text.setAttribute("y", box.y + 12);
  text.classList.add("group-label");
  text.textContent = entity.id;
  overlay.append(text);
  bindShape(overlay, entity.id);
  svg.append(overlay);
  groupShapes.set(entity.id, [overlay]);
}

function draw(source) {
  const canvas = byId("canvas");
  canvas.replaceChildren();
  canvas.classList.remove("measurement-gallery");
  shapes = new Map();
  groupShapes = new Map();
  prepareRelations(null);
  const svg = source ? safeSvg(source) : null;
  if (!svg) {
    canvas.textContent = "This step has no drawable geometry. Its result is shown below.";
    return;
  }
  expandMeasuredLabels(svg);
  canvas.classList.toggle("measurement-gallery", svg.hasAttribute("data-measurement-gallery"));
  canvas.append(svg);
  for (const element of svg.querySelectorAll("g[id]")) {
    const id = element.id;
    element.setAttribute("data-entity", id);
    element.removeAttribute("id");
    shapes.set(id, element);
    bindShape(element, id);
    const title = document.createElementNS(svg.namespaceURI, "title");
    title.textContent = sourceDescription(id);
    element.prepend(title);
  }
  for (const element of svg.querySelectorAll("[id]")) element.removeAttribute("id");
  svg.removeAttribute("id");
  if (!svg.hasAttribute("data-measurement-gallery")) {
    for (const entity of dump.entities.filter(item => item.members)) drawGroup(svg, entity);
  }
  prepareRelations(svg);
  toggleGroups();
  highlight();
}

function toggleGroups() {
  for (const elements of groupShapes.values()) {
    elements.forEach(element => { element.style.display = byId("show-groups").checked ? "" : "none"; });
  }
}

function highlight(ids = []) {
  if (!ids.length) showRelations();
  const active = new Set(ids.flatMap(id => [...members(id)]));
  for (const [id, shape] of shapes) shape.classList.toggle("hovered", active.has(id));
  for (const [id, elements] of groupShapes) {
    elements.forEach(element => element.classList.toggle("hovered", ids.includes(id)));
  }
}

function bindRelation(element, relation) {
  element.addEventListener("pointerenter", () => {
    element.classList.add("hovered");
    byId("hover").textContent = `${label(relation.kind)}: ${relation.source}`;
    highlight(relation.ids);
    showRelations([relation]);
    showSourceLine(relation.span?.line);
  });
  element.addEventListener("pointerleave", () => {
    element.classList.remove("hovered");
    highlight();
    showSourceLine();
  });
}

function connects(relation, first, second) {
  const ids = relation.ids || [];
  return ids.some(a => first.has(a) && ids.some(b => a !== b && second.has(b)));
}

function updateRelations() {
  const first = byId("first").value;
  const second = byId("second").value;
  const list = byId("relations");
  list.replaceChildren();
  const matching = first && second ? dump.relations.filter(relation => connects(relation, members(first), members(second))) : [];
  byId("relation-note").textContent = matching.length ? `${matching.length} relationship(s), including group members.` : "No recorded relationship between these selections.";
  for (const relation of matching) {
    const item = document.createElement("li");
    item.textContent = `${label(relation.kind)}: ${relation.source} [${relation.ids.join(", ")}]`;
    bindRelation(item, relation);
    list.append(item);
  }
  highlight();
  showSourceLine();
}

function showStep(index) {
  byId("tooltip").hidden = true;
  showSourceLine();
  for (const item of byId("relations").children) item.classList.remove("hovered");
  position = index;
  const step = dump.steps[index];
  byId("position").value = visible.indexOf(index);
  byId("counter").textContent = `${index + 1} / ${dump.steps.length}`;
  byId("step-title").textContent = label(step.name);
  byId("step-note").textContent = step.data?.preview_kind === "measurements" ? "Measured glyphs and bounds before placement. Scroll to inspect each label." : step.svg ? "Snapshot recorded at this step. Hover to inspect source names." : "Structured result recorded at this step.";
  byId("data").textContent = JSON.stringify(step.data, null, 2);
  draw(step.svg);
  const current = visible.indexOf(index);
  byId("previous").disabled = current <= 0;
  byId("next").disabled = current < 0 || current === visible.length - 1;
  for (const button of byId("steps").children) button.setAttribute("aria-current", Number(button.dataset.index) === index);
}

function filterSteps() {
  const kind = byId("kind").value;
  visible = dump.steps.map((_, index) => index).filter(index => {
    const step = dump.steps[index];
    if (kind === "visual") return !!step.svg;
    if (kind === "solver") return /solver|settling|solve_pass/.test(step.name);
    return true;
  });
  byId("steps").replaceChildren();
  for (const index of visible) {
    const button = document.createElement("button");
    button.dataset.index = index;
    button.textContent = `${String(index + 1).padStart(2, "0")}  ${label(dump.steps[index].name)}`;
    button.addEventListener("click", () => showStep(index));
    byId("steps").append(button);
  }
  byId("position").max = Math.max(0, visible.length - 1);
  byId("position").disabled = !visible.length;
  if (!visible.length) {
    byId("steps").textContent = "No steps match this filter.";
    byId("previous").disabled = true;
    byId("next").disabled = true;
    return;
  }
  showStep(visible.includes(position) ? position : visible[0]);
}

function initialize() {
  byId("filename").textContent = dump.path;
  byId("status").textContent = dump.result.exit_code === 0 ? "Render completed" : "Render failed · partial trace";
  byId("position").max = dump.steps.length - 1;
  for (const id of ["first", "second"]) {
    const empty = document.createElement("option");
    empty.value = "";
    empty.textContent = "Choose an object or group";
    byId(id).append(empty);
    for (const entity of dump.entities) {
      const option = document.createElement("option");
      option.value = entity.id;
      option.textContent = `${entity.id} (${entity.kind})`;
      byId(id).append(option);
    }
    byId(id).addEventListener("change", updateRelations);
  }
  dump.source.split("\n").forEach((text, index) => {
    const line = document.createElement("span");
    line.className = "source-line";
    line.id = `line-${index + 1}`;
    const number = document.createElement("span");
    number.className = "source-number";
    number.textContent = index + 1;
    line.append(number, document.createTextNode(text));
    byId("source").append(line);
  });
  byId("kind").addEventListener("change", filterSteps);
  byId("show-groups").addEventListener("change", toggleGroups);
  byId("position").addEventListener("input", event => showStep(visible[Number(event.target.value)]));
  byId("previous").addEventListener("click", () => showStep(visible[visible.indexOf(position) - 1]));
  byId("next").addEventListener("click", () => showStep(visible[visible.indexOf(position) + 1]));
  filterSteps();
  updateRelations();
}
initialize();
