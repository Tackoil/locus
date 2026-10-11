const { readdirSync, mkdtempSync, writeFileSync, rmSync } = require("node:fs");
const { join, resolve } = require("node:path");
const { tmpdir } = require("node:os");
const { spawnSync } = require("node:child_process");
const assert = require("node:assert/strict");

const root = resolve(__dirname, "../../..");

function sources(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sources(path);
    return entry.name.endsWith(".locus") ? [path] : [];
  });
}

function run(args) {
  const result = spawnSync("tree-sitter", args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  assert.equal(result.status, 0, result.error?.message ?? result.stdout + result.stderr);
  return result.stdout;
}

function checkEditing() {
  const directory = mkdtempSync(join(tmpdir(), "locus-grammar-"));
  try {
    const path = join(directory, "editing.locus");
    for (const ending of ["\n", "\r\n", "\r"]) {
      writeFileSync(path, ["locus 0 beta 6", "point A()", "point B = A"].join(ending));
      run(["parse", "--quiet", path]);
    }
    // Malformed input must not consume the following valid declaration.
    for (const incomplete of ['meta title = "unfinished', "point A = point(", 'label a = label(A, "unfinished', "label a = label(A, $unfinished"]) {
      writeFileSync(path, `locus 0 beta 6\n${incomplete}\npoint B()\n`);
      const result = spawnSync("tree-sitter", ["parse", "--xml", path], { encoding: "utf8" });
      assert.equal(result.status, 1, "Incomplete input should contain a syntax error");
      assert.match(result.stdout, /<object_declaration[^>]*>\s*<kind[^>]*>\s*point\s*<\/kind>\s*<identifier[^>]*>B<\/identifier>/);
      assert.match(result.stdout, /<ERROR|<MISSING|\(MISSING /);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

const files = [
  ...sources(join(root, "spec/test")),
  ...sources(join(root, "fixtures/beta6")),
  ...sources(join(root, "assets")),
];
run(["parse", "--quiet", ...files]);
for (const name of ["highlights", "brackets", "indents", "outline"]) {
  run(["query", "--quiet", join(root, "ecosystem/zed/languages/locus", `${name}.scm`), ...files]);
}
// Zed requires an @indent capture; a syntactically valid empty query is rejected.
const indents = run(["query", "--captures",
  join(root, "ecosystem/zed/languages/locus/indents.scm"),
  join(root, "spec/test/composition/flowchart_agent_loop.locus"),
]);
for (const capture of ["indent", "start", "end"]) {
  assert.match(indents, new RegExp(`capture: \\d+ - ${capture},`));
}
checkEditing();
console.log(`Parsed ${files.length} existing Locus files; validated four Zed queries and editing recovery.`);
