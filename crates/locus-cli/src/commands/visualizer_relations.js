// Relationship graphics are temporary and use the current snapshot's geometry.
let relationLayer = null;
let relationSvg = null;
let sceneBounds = new Map();
let sceneTransform = null;

function prepareRelations(svg) {
  relationSvg = svg;
  relationLayer = null;
  sceneBounds = new Map();
  sceneTransform = null;
  if (!svg || svg.hasAttribute("data-measurement-gallery")) return;
  for (let index = position; index >= 0; index--) {
    const objects = dump.steps[index].data?.objects;
    if (!Array.isArray(objects)) continue;
    sceneBounds = new Map(objects.filter(object => object.bounds).map(object => [object.id, object.bounds]));
    break;
  }
  sceneTransform = inferSceneTransform();
  relationLayer = document.createElementNS(svg.namespaceURI, "g");
  relationLayer.classList.add("relation-layer");
  svg.append(relationLayer);
}

function inferSceneTransform() {
  const bounded = new Set(["rect", "rounded_rect", "diamond", "polygon", "circle", "ellipse", "segment", "bspline"]);
  for (const [id, shape] of shapes) {
    const world = sceneBounds.get(id);
    if (!world || !bounded.has(entities.get(id)?.kind)) continue;
    const box = shape.getBBox();
    const width = world.max.x - world.min.x;
    const height = world.max.y - world.min.y;
    const scale = width > 1e-9 ? box.width / width : box.height / height;
    if (!Number.isFinite(scale) || scale <= 0) continue;
    return {scale, x: box.x - world.min.x * scale, y: box.y + world.max.y * scale};
  }
  for (const [id, shape] of shapes) {
    const world = sceneBounds.get(id);
    if (!world || entities.get(id)?.kind !== "point") continue;
    const center = boxCenter(shape.getBBox());
    return {scale: 32, x: center.x - world.min.x * 32, y: center.y + world.min.y * 32};
  }
  return null;
}

function figureBounds(id) {
  const visible = groupBounds(id);
  if (visible) return visible;
  const world = sceneBounds.get(id);
  if (!world || !sceneTransform) return null;
  const {scale, x, y} = sceneTransform;
  return {x: x + world.min.x * scale, y: y - world.max.y * scale,
    width: (world.max.x - world.min.x) * scale, height: (world.max.y - world.min.y) * scale};
}

function boxCenter(box) {
  return {x: box.x + box.width / 2, y: box.y + box.height / 2};
}

function expressionReferences(expression) {
  if (!expression || typeof expression !== "object") return [];
  if (expression.type === "ref") return [expression.name];
  return Object.values(expression).flatMap(expressionReferences);
}

function relationParticipants(relation) {
  const ordered = expressionReferences(relation.expression);
  const ids = [...new Set(ordered.length ? ordered : relation.ids)];
  const name = relation.expression?.name?.name;
  const expanded = name === "row" || name === "column"
    ? ids.flatMap(id => [...members(id)].filter(member => !entities.get(member)?.members)) : ids;
  return [...new Set(expanded)].map(id => ({id, box: figureBounds(id)})).filter(item => item.box);
}

function relatedConstraints(id) {
  const first = byId("first").value;
  const second = byId("second").value;
  return dump.relations.filter(relation => {
    if (relation.kind !== "constraint") return false;
    // Sharing a member does not make a constraint about this object or group.
    if (!relation.ids.includes(id)) return false;
    return !first || !second || connects(relation, members(first), members(second));
  });
}

function relationCaption(relation) {
  const text = relation.expression?.type === "call" ? relation.expression.name.name : relation.source;
  return text.length > 70 ? `${text.slice(0, 67)}…` : text;
}

function svgElement(name, attributes) {
  const element = document.createElementNS(relationSvg.namespaceURI, name);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
  return element;
}

function drawRelation(relation, lane) {
  const participants = relationParticipants(relation);
  if (!participants.length) return;
  const overlay = svgElement("g", {"data-relation-kind": relation.kind});
  const title = svgElement("title", {});
  title.textContent = relation.source;
  overlay.append(title);
  const points = participants.map(item => boxCenter(item.box));
  points.forEach(point => overlay.append(svgElement("circle", {cx: point.x, cy: point.y, r: 3})));
  for (let index = 1; index < points.length; index++) {
    const start = points[index - 1];
    const end = points[index];
    const lift = 18 + (lane % 6) * 12;
    const path = `M ${start.x} ${start.y} Q ${(start.x + end.x) / 2} ${Math.min(start.y, end.y) - lift} ${end.x} ${end.y}`;
    overlay.append(svgElement("path", {d: path}));
  }
  const center = points.reduce((sum, point) => ({x: sum.x + point.x / points.length, y: sum.y + point.y / points.length}), {x: 0, y: 0});
  const text = svgElement("text", {x: center.x + 8, y: center.y - 12 - (lane % 6) * 12});
  text.textContent = relationCaption(relation);
  overlay.append(text);
  relationLayer.append(overlay);
}

function showRelations(relations = []) {
  if (!relationLayer) return;
  relationLayer.replaceChildren();
  relations.forEach(drawRelation);
}
