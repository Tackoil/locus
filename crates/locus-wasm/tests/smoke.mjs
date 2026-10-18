import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import init, { render } from '../../../target/wasm/pkg/locus.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const examples = path.join(root, 'examples/demonstrations');
await init({ module_or_path: await readFile(path.join(root, 'target/wasm/pkg/locus_bg.wasm')) });
const files = process.argv.includes('--all')
  ? (await readdir(examples)).filter(file => file.endsWith('.locus')).sort()
  : ['point_relations', 'flowchart_agent_loop', 'euler_nine_point', 'sequence_check'].map(name => `${name}.locus`);

for (const file of files) {
  const source = await readFile(path.join(examples, file), 'utf8');
  const result = JSON.parse(render(source));
  assert.equal(result.ok, true, `${file}: ${JSON.stringify(result.diagnostics)}`);
  assert.match(result.svg, /<svg\b/);
  assert.equal(result.assertions.passed, true, `${file}: assertions failed`);
  if (file === 'point_relations.locus') {
    assert.equal(render(source), render(source), 'repeated WASM renders are deterministic');
    const failed = JSON.parse(render(source.replace('assert area(tri) == 6', 'assert area(tri) == 7')));
    assert.equal(failed.ok, true);
    assert.equal(failed.assertions.passed, false);
    if (process.env.LOCUS_BIN) {
      const native = execFileSync(process.env.LOCUS_BIN, ['render', path.join(examples, file), '--stdout'], { encoding: 'utf8' });
      assert.equal(result.svg, native, 'triangle SVG matches the native CLI byte for byte');
    }
  }
  console.log(`WASM verified: ${file}`);
}
for (const source of ['bad header', 'locus 0 beta 6\npoint A = midpoint(B, C)\n']) {
  const result = JSON.parse(render(source));
  assert.equal(result.ok, false);
  assert.equal(result.svg, null);
  assert.ok(result.diagnostics[0].span.line > 0);
}
const cjk = JSON.parse(render('locus 0 beta 6\npoint A()\nlabel title(A, "中文 日本語 한국어")\n'));
assert.equal(cjk.ok, true);
assert.match(cjk.svg, /<path\b/);
const invalidTex = JSON.parse(render('locus 0 beta 6\npoint A()\nlabel title(A, $\\notalocuscommand$)\n'));
assert.equal(invalidTex.ok, false);
assert.equal(invalidTex.diagnostics[0].code, 'E414');
const nested = JSON.parse(render(`locus 0 beta 6\nnum value = free ${'('.repeat(5000)}1${')'.repeat(5000)}\n`));
assert.equal(nested.ok, false);
assert.equal(nested.svg, null);
assert.ok(nested.diagnostics.some(diagnostic => diagnostic.code === 'E161'));
assert.equal(JSON.parse(render('locus 0 beta 6\npoint A()\n')).ok, true, 'renderer recovers after errors');
console.log('WASM diagnostics, assertions, CJK, TeX errors, nesting limits, and recovery verified.');
