// Run with: node --test crates/locus-cli/tests/visualizer.test.cjs
// Pure viewer logic runs in an isolated context with no browser or network.
const { readFileSync } = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const assert = require("node:assert/strict");
const { test } = require("node:test");

function viewer() {
  const dump = {
    path: "scene.locus",
    source: "",
    entities: [
      {id: "first", kind: "circle", source_name: "shape", span: {line: 4, col: 1}},
      {id: "second", kind: "circle"},
      {id: "left", kind: "group", members: ["first"]},
      {id: "right", kind: "instance", members: ["second"]},
      {id: "all", kind: "group", members: ["left", "right"]},
      {id: "cycle", kind: "group", members: ["cycle"]},
    ],
    relations: [], steps: [{name: "parse"}, {name: "render_output", svg: "<svg/>"}, {name: "result"}],
    result: {exit_code: 0},
  };
  const context = vm.createContext({document: {getElementById: () => ({textContent: JSON.stringify(dump)})}});
  const source = readFileSync(path.join(__dirname, "../src/commands/visualizer.js"), "utf8");
  const relations = readFileSync(path.join(__dirname, "../src/commands/visualizer_relations.js"), "utf8");
  vm.runInContext(relations + "\n" + source.replace(/^initialize\(\);$/m, ""), context);
  return expression => vm.runInContext(expression, context);
}

test("nested groups resolve to member identities and tolerate cycles", () => {
  const run = viewer();
  assert.equal(run('members("all").has("first") && members("all").has("second")'), true);
  assert.equal(run('members("left").has("second")'), false);
  assert.equal(run('members("cycle").size'), 1);
});

test("relations connect objects through either selected group's descendants", () => {
  const run = viewer();
  assert.equal(run('connects({ids:["first", "second"]}, members("left"), members("right"))'), true);
  assert.equal(run('connects({ids:["first", "second"]}, members("right"), members("left"))'), true);
  assert.equal(run('connects({ids:["first", "unrelated"]}, members("left"), members("right"))'), false);
  assert.equal(run('connects({ids:["left", "first"]}, members("left"), members("first"))'), true);
});

test("hover descriptions retain original names, locations, and nested membership", () => {
  const run = viewer();
  const description = run('sourceDescription("first")');
  assert.match(description, /first \(source: shape\)/);
  assert.match(description, /scene\.locus:4:1/);
  assert.match(description, /Groups: left, all/);
  assert.equal(run('sourceDescription("missing")'), "missing");
});

test("the viewer opens at the final available image", () => {
  assert.equal(viewer()("position"), 1);
});

test("measured glyphs are inserted into every snapshot and cached by identity", () => {
  const run = viewer();
  run(`
    measuredLabels.set("caption", "measured-svg");
    var parses = 0;
    safeSvg = source => { parses++; return {childNodes:[{cloneNode: () => ({source})}]}; };
    var drawn = [];
    var placeholder = {getAttribute: () => "caption", append: (...nodes) => drawn.push(...nodes)};
    var svg = {querySelectorAll: () => [placeholder]};
    expandMeasuredLabels(svg);
    expandMeasuredLabels(svg);
  `);
  assert.equal(run("drawn.length"), 2);
  assert.equal(run("drawn[0].source"), "measured-svg");
  assert.equal(run("parses"), 1);
});

test("missing glyph assets leave old dumps usable", () => {
  const run = viewer();
  assert.equal(run(`
    var appended = 0;
    expandMeasuredLabels({querySelectorAll: () => [{getAttribute: () => "unknown", append: () => appended++}]});
    appended;
  `), 0);
});

function hoverViewer() {
  const run = viewer();
  run(`
    function makeElement() {
      const classes = new Set();
      const events = {};
      return {classes, events,
        classList: {
          toggle: (name, enabled) => enabled ? classes.add(name) : classes.delete(name),
          add: name => classes.add(name), remove: name => classes.delete(name),
        },
        addEventListener: (name, handler) => { events[name] = handler; },
      };
    }
    var nodes = {first:{value:"first"}, second:{value:"second"}, hover:{textContent:""}, tooltip:{hidden:false}};
    nodes["line-4"] = makeElement();
    document.getElementById = id => nodes[id];
    document.querySelectorAll = () => [nodes["line-4"]];
    var firstShape = makeElement(), secondShape = makeElement();
    var leftGroup = makeElement(), rightGroup = makeElement(), allGroup = makeElement();
    shapes = new Map([["first", firstShape], ["second", secondShape]]);
    groupShapes = new Map([["left", [leftGroup]], ["right", [rightGroup]], ["all", [allGroup]]]);
  `);
  return run;
}

test("selected objects stay unhighlighted until hovered", () => {
  const run = hoverViewer();
  run("highlight();");
  assert.equal(run("firstShape.classes.size + secondShape.classes.size"), 0);
  assert.equal(run("leftGroup.classes.size + rightGroup.classes.size + allGroup.classes.size"), 0);
});

test("group hover highlights its members and only the hovered group frame", () => {
  const run = hoverViewer();
  run('highlight(["left"]);');
  assert.equal(run('firstShape.classes.has("hovered")'), true);
  assert.equal(run('secondShape.classes.has("hovered")'), false);
  assert.equal(run('leftGroup.classes.has("hovered")'), true);
  assert.equal(run('allGroup.classes.has("hovered")'), false);
  run('highlight(["all"]);');
  assert.equal(run('firstShape.classes.has("hovered") && secondShape.classes.has("hovered")'), true);
  assert.equal(run('allGroup.classes.has("hovered")'), true);
  assert.equal(run('leftGroup.classes.has("hovered") || rightGroup.classes.has("hovered")'), false);
  run("highlight();");
  assert.equal(run("firstShape.classes.size + secondShape.classes.size + allGroup.classes.size"), 0);
});

test("relation hover highlights its participants and clears them on pointer leave", () => {
  const run = hoverViewer();
  run(`
    var item = makeElement();
    bindRelation(item, {kind:"constraint", source:"align(first, second)", ids:["left", "second"], span:{line:4}});
    item.events.pointerenter();
  `);
  assert.equal(run('item.classes.has("hovered")'), true);
  assert.equal(run('firstShape.classes.has("hovered") && secondShape.classes.has("hovered")'), true);
  assert.equal(run('leftGroup.classes.has("hovered")'), true);
  assert.equal(run('nodes["line-4"].classes.has("active")'), true);
  run("item.events.pointerleave();");
  assert.equal(run("item.classes.size + firstShape.classes.size + secondShape.classes.size + leftGroup.classes.size"), 0);
  assert.equal(run('nodes["line-4"].classes.has("active")'), false);
});

test("leaving a group clears hover even when it is still selected", () => {
  const run = hoverViewer();
  run('bindShape(leftGroup, "left"); highlight(["left"]); leftGroup.events.pointerleave();');
  assert.equal(run("leftGroup.classes.size + firstShape.classes.size"), 0);
  assert.equal(run("nodes.first.value"), "first");
  assert.equal(run("nodes.tooltip.hidden"), true);
});

function figureViewer() {
  const run = hoverViewer();
  run(`
    function makeSvg(tag) {
      return {...makeElement(), tag, attributes:{}, children:[], namespaceURI:"http://www.w3.org/2000/svg",
        setAttribute(name, value) { this.attributes[name] = String(value); },
        append(...children) { this.children.push(...children); },
        replaceChildren(...children) { this.children = children; },
      };
    }
    document.createElementNS = (_, tag) => makeSvg(tag);
    firstShape.getBBox = () => ({x:10, y:20, width:20, height:10});
    secondShape.getBBox = () => ({x:100, y:20, width:20, height:10});
    relationSvg = makeSvg("svg");
    relationLayer = makeSvg("g");
  `);
  return run;
}

test("hovering a relation draws a named connector on the figure and leaving removes it", () => {
  const run = figureViewer();
  run(`
    var item = makeElement();
    bindRelation(item, {kind:"constraint", source:"align_center_y(first, second)", ids:["first", "second"],
      expression:{type:"call", name:{name:"align_center_y"}, args:[{type:"ref", name:"first"}, {type:"ref", name:"second"}]}});
  `);
  assert.equal(run("relationLayer.children.length"), 0);
  run("item.events.pointerenter();");
  assert.equal(run("relationLayer.children.length"), 1);
  assert.equal(run('relationLayer.children[0].children.filter(node => node.tag === "path").length'), 1);
  assert.equal(run('relationLayer.children[0].children.find(node => node.tag === "text").textContent'), "align_center_y");
  assert.equal(run('relationLayer.children[0].children.filter(node => node.tag === "circle").map(node => node.attributes.cx).join(",")'), "20,110");
  run("item.events.pointerleave();");
  assert.equal(run("relationLayer.children.length"), 0);
});

test("object hover excludes constraints authored only on a containing group", () => {
  const run = figureViewer();
  run(`
    nodes.first.value = "";
    nodes.second.value = "";
    dump.relations.push(
      {kind:"constraint", ids:["all"], source:"row(all)"},
      {kind:"constraint", ids:["first", "second"], source:"align(first, second)"},
      {kind:"dependency", ids:["first", "second"], source:"definition"}
    );
  `);
  assert.equal(run('relatedConstraints("first").map(relation => relation.source).join(",")'), "align(first, second)");
});

test("group row relations connect member shapes in authored order", () => {
  const run = figureViewer();
  assert.equal(run(`relationParticipants({kind:"constraint", ids:["all"], source:"row(all)",
    expression:{type:"call", name:{name:"row"}, args:[{type:"ref", name:"all"}]}})
    .map(item => item.id).join(",")`), "first,second");
});

test("relation endpoints include invisible construction points from the current snapshot", () => {
  const run = figureViewer();
  run(`
    sceneBounds = new Map([
      ["first", {min:{x:0,y:0}, max:{x:2,y:1}}],
      ["hidden", {min:{x:5,y:-2}, max:{x:5,y:-2}}]
    ]);
    firstShape.getBBox = () => ({x:10,y:20,width:64,height:32});
    sceneTransform = inferSceneTransform();
  `);
  assert.equal(run('figureBounds("hidden").x'), 170);
  assert.equal(run('figureBounds("hidden").y'), 116);
  assert.equal(run('figureBounds("unknown")'), null);
});

test("single-object constraints still draw their relation caption", () => {
  const run = figureViewer();
  run('showRelations([{kind:"constraint", ids:["first"], source:"first.width == 4"}]);');
  assert.equal(run('relationLayer.children[0].children.find(node => node.tag === "text").textContent'), "first.width == 4");
  run("highlight();");
  assert.equal(run("relationLayer.children.length"), 0);
});

test("hovering headings excludes constraints on shared members and enclosing instances", () => {
  const run = figureViewer();
  run(`
    nodes.first.value = "";
    nodes.second.value = "";
    entities.set("headings", {id:"headings", members:["approved.heading", "declined.heading"]});
    entities.set("approved", {id:"approved", members:["approved.heading", "approved.payment"]});
    entities.set("declined", {id:"declined", members:["declined.heading", "declined.payment"]});
    entities.set("messages", {id:"messages", members:["approved", "declined"]});
    dump.relations.push(
      {kind:"constraint", ids:["headings", "guides"], source:"avoid(headings, guides)"},
      {kind:"constraint", ids:["approved.heading", "approved.payment"], source:"gap_y(heading, payment)"},
      {kind:"constraint", ids:["alt.frame", "approved", "declined"], source:"contain(frame, upper, lower)"},
      {kind:"constraint", ids:["messages", "web_active.shape"], source:"trim_endpoints(paths, boundary)"},
      {kind:"constraint", ids:["approved.heading", "alt.frame"], source:"inset_left(upper_heading, frame)"}
    );
  `);
  assert.equal(run('relatedConstraints("headings").map(relation => relation.source).join(";")'), "avoid(headings, guides)");
  assert.equal(run('relatedConstraints("approved.heading").map(relation => relation.source).join(";")'),
    "gap_y(heading, payment);inset_left(upper_heading, frame)");
});

test("two groups sharing a member do not inherit each other's relations", () => {
  const run = figureViewer();
  run(`
    nodes.first.value = "";
    nodes.second.value = "";
    entities.set("overlap", {id:"overlap", members:["first"]});
    dump.relations.push(
      {kind:"constraint", ids:["overlap", "second"], source:"avoid(overlap, second)"},
      {kind:"constraint", ids:["left", "second"], source:"avoid(left, second)"}
    );
  `);
  assert.equal(run('relatedConstraints("left").map(relation => relation.source).join(";")'), "avoid(left, second)");
  assert.equal(run('relatedConstraints("overlap").map(relation => relation.source).join(";")'), "avoid(overlap, second)");
});
