import assert from 'node:assert/strict';
import { after, beforeEach, test } from 'node:test';
import { createRenderer } from '../src/playground-renderer.js';
import { diagnosticItems, resultSummary } from '../src/playground-diagnostics.js';
import { fileName, readDraft, saveDraft } from '../src/playground-files.js';

const previousWorker = globalThis.Worker;
const previousStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
class WorkerStub {
  static instances = [];
  constructor(url, options) {
    this.url = url;
    this.options = options;
    WorkerStub.instances.push(this);
  }
  postMessage(data) { this.request = data; }
  terminate() { this.terminated = true; }
  respond(type, extra = {}) { this.onmessage({ data: { id: this.request.id, type, ...extra } }); }
}
globalThis.Worker = WorkerStub;
after(() => {
  if (previousWorker === undefined) delete globalThis.Worker;
  else globalThis.Worker = previousWorker;
  if (previousStorage === undefined) delete globalThis.localStorage;
  else Object.defineProperty(globalThis, 'localStorage', previousStorage);
});
beforeEach(() => { WorkerStub.instances = []; });

function setup(t) {
  const phases = [];
  const results = [];
  const errors = [];
  const renderer = createRenderer({ onPhase: value => phases.push(value), onResult: value => results.push(value), onError: value => errors.push(value) });
  t.after(() => renderer.cancel());
  return { renderer, phases, results, errors };
}

test('loads a module worker lazily and reuses it after successful renders', t => {
  const { renderer, phases, results } = setup(t);
  assert.equal(WorkerStub.instances.length, 0);
  renderer.start('first source');
  const worker = WorkerStub.instances[0];
  assert.match(worker.url.href, /playground-worker\.js$/);
  assert.equal(worker.options.type, 'module');
  assert.equal(worker.request.source, 'first source');
  worker.respond('rendering');
  worker.respond('result', { result: { ok: true, svg: '<svg/>' } });
  assert.deepEqual(results, [{ ok: true, svg: '<svg/>' }]);
  assert.match(phases[0], /Loading/);
  renderer.start('second source');
  assert.equal(WorkerStub.instances.length, 1);
  assert.equal(worker.request.source, 'second source');
});

test('cancellation ignores stale results and errors from the terminated worker', t => {
  const { renderer, results, errors } = setup(t);
  renderer.start('old');
  const old = WorkerStub.instances[0];
  renderer.cancel();
  renderer.start('new');
  assert.equal(old.terminated, true);
  old.respond('result', { result: { ok: true } });
  old.onerror();
  assert.deepEqual(results, []);
  assert.deepEqual(errors, []);
  WorkerStub.instances[1].respond('result', { result: { ok: false } });
  assert.deepEqual(results, [{ ok: false }]);
});

test('runtime traps and loading errors discard the instance and allow retries', t => {
  const { renderer, errors } = setup(t);
  renderer.start('trap');
  const first = WorkerStub.instances[0];
  first.respond('fatal', { error: 'unreachable' });
  assert.equal(first.terminated, true);
  assert.match(errors[0], /unreachable/);
  renderer.start('retry');
  WorkerStub.instances[1].onerror();
  assert.equal(WorkerStub.instances[1].terminated, true);
  renderer.start('valid');
  assert.equal(WorkerStub.instances.length, 3);
});

test('timeout cancels a render and permits a fresh worker', t => {
  let timeout;
  t.mock.method(globalThis, 'setTimeout', (callback, delay) => {
    assert.equal(delay, 30000);
    timeout = callback;
    return 1;
  });
  t.mock.method(globalThis, 'clearTimeout', () => {});
  const { renderer, errors } = setup(t);
  renderer.start('slow source');
  timeout();
  assert.equal(WorkerStub.instances[0].terminated, true);
  assert.match(errors[0], /30 seconds/);
  renderer.start('retry');
  assert.equal(WorkerStub.instances.length, 2);
});

test('worker construction errors become actionable feedback', t => {
  t.mock.method(globalThis, 'Worker', () => { throw new Error('worker blocked'); });
  const { renderer, errors } = setup(t);
  renderer.start('source');
  assert.match(errors[0], /Could not start the renderer/);
});

const span = { line: 3, col: 8, byte_start: 20, byte_end: 22 };
test('diagnostics retain locations, notes, suggestions, and failed assertion values', () => {
  const items = diagnosticItems({
    diagnostics: [{ code: 'E100', message: 'Unknown object', span, notes: ['A note'], suggestion: 'Try point A()' }],
    warnings: [{ kind: 'label_collision', labels: ['a', 'b'], spans: [span] }, { kind: 'crowding', first: 'A', second: 'B', distance: 0.1 }],
    assertions: { assertions: [{ index: 0, passed: false, span, actuals: [{ span, value: 6 }] }, { index: 1, passed: true }] },
  });
  assert.equal(items.length, 4);
  assert.equal(items[0].span, span);
  assert.equal(items[0].details, 'A note Try point A()');
  assert.match(items[1].text, /Labels need more space: a, b/);
  assert.equal(items[1].span, span);
  assert.match(items[2].text, /A and B/);
  assert.equal(items[3].details, 'Column 8: 6');
});

test('summaries distinguish errors from rendered SVGs with failed assertions', () => {
  assert.match(resultSummary({ ok: false }), /Could not render/);
  assert.match(resultSummary({ ok: true, warnings: [{}], assertions: { assertions: [{ passed: true }, { passed: false }] } }), /1\/2 assertions passed · 1 warning/);
});

test('drafts round-trip and tolerate disabled storage and invalid data', () => {
  const store = new Map();
  globalThis.localStorage = { getItem: key => store.get(key) ?? null, setItem: (key, value) => store.set(key, value) };
  assert.equal(readDraft(), null);
  saveDraft('中文 source', 'triangle.locus');
  assert.deepEqual(readDraft(), { source: '中文 source', name: 'triangle.locus' });
  store.set('locus-playground-draft-v1', '{bad json');
  assert.equal(readDraft(), null);
  store.set('locus-playground-draft-v1', '{"source":17,"name":"x"}');
  assert.equal(readDraft(), null);
  globalThis.localStorage = { getItem() { throw new Error('blocked'); }, setItem() { throw new Error('blocked'); } };
  assert.equal(readDraft(), null);
  assert.doesNotThrow(() => saveDraft('source', 'name'));
});

test('export names preserve Unicode and replace unsafe filename characters', () => {
  assert.equal(fileName('a diagram.locus', '.svg'), 'a_diagram.svg');
  assert.equal(fileName('中文.locus', '.locus'), '中文.locus');
});
