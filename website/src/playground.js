import { createRenderer } from './playground-renderer.js';
import { readDraft, saveDraft, fileName, downloadSource } from './playground-files.js';
import { showDiagnostics, resultSummary } from './playground-diagnostics.js';
import { createCodeEditor } from './playground-editor.js';

export function createPlayground(demos) {
  const $ = id => document.getElementById(`playground-${id}`);
  const source = $('source');
  let initialized = false;
  let active = false;
  let busy = false;
  let dirty = true;
  let name = 'untitled.locus';
  let openedExample = '';
  let debounce;
  let svgURL;
  let editor;
  const renderer = createRenderer({ onPhase: phase, onResult: rendered, onError: failed });

  function setBusy(value) {
    busy = value;
    $('cancel').hidden = !value;
    $('render').disabled = value;
    $('preview').setAttribute('aria-busy', value);
  }

  function phase(message) {
    setBusy(true);
    $('status').textContent = message;
  }

  function failed(message) {
    setBusy(false);
    dirty = true;
    $('status').textContent = message;
  }

  function selectSpan(span) {
    const bytes = new TextEncoder().encode(source.value);
    const decoder = new TextDecoder();
    const start = decoder.decode(bytes.subarray(0, span.byte_start)).length;
    const end = decoder.decode(bytes.subarray(0, span.byte_end)).length;
    source.focus();
    source.setSelectionRange(start, Math.max(start + 1, end));
    source.scrollTop = Math.max(0, (span.line - 4) * parseFloat(getComputedStyle(source).lineHeight));
    editor.syncScroll();
  }

  function rendered(result) {
    setBusy(false);
    $('status').textContent = resultSummary(result);
    showDiagnostics($('diagnostics'), result, selectSpan);
    if (!result.ok) {
      $('preview-caption').textContent = svgURL
        ? 'Source has errors. Preview shows the last successful render.'
        : 'Source has errors. Fix the diagnostics to render a preview.';
      return;
    }
    if (svgURL) URL.revokeObjectURL(svgURL);
    svgURL = URL.createObjectURL(new Blob([result.svg], { type: 'image/svg+xml' }));
    $('image').src = svgURL;
    $('image').hidden = false;
    $('placeholder').hidden = true;
    $('download').href = svgURL;
    $('download').download = fileName(name, '.svg');
    $('download').hidden = false;
    $('preview-caption').textContent = result.assertions.passed ? 'Preview is up to date.' : 'Preview is up to date. Some assertions failed.';
  }

  function render() {
    clearTimeout(debounce);
    if (busy) renderer.cancel();
    dirty = false;
    $('download').hidden = true;
    $('diagnostics').hidden = true;
    renderer.start(source.value);
  }

  function changed() {
    clearTimeout(debounce);
    if (busy) renderer.cancel();
    setBusy(false);
    dirty = true;
    $('download').hidden = true;
    $('diagnostics').hidden = true;
    $('status').textContent = 'Source changed. Ready to render.';
    $('preview-caption').textContent = 'Preview needs rendering.';
    $('lines').textContent = `${editor.update()} lines`;
    saveDraft(source.value, name);
    if (active && $('auto').checked) debounce = setTimeout(render, 600);
  }

  function load(text, filename, example = '') {
    source.value = text;
    source.scrollTop = 0;
    source.scrollLeft = 0;
    name = filename;
    openedExample = example;
    $('filename').textContent = name;
    $('example').value = example;
    changed();
    render();
  }

  function loadExample(id) {
    const demo = demos.find(item => item.id === id);
    if (demo) load(demo.source, `${demo.id}.locus`, demo.id);
  }

  async function openFile(event) {
    const file = event.target.files[0];
    if (!file) return;
    try {
      load(await file.text(), fileName(file.name, '.locus'));
      history.replaceState(null, '', '#playground');
    }
    catch { $('status').textContent = 'Could not read this file. Try opening it again.'; }
    event.target.value = '';
  }

  function initialize() {
    editor = createCodeEditor(source);
    for (const demo of demos) {
      const option = document.createElement('option');
      option.value = demo.id;
      option.textContent = demo.name;
      $('example').append(option);
    }
    source.addEventListener('input', () => {
      openedExample = '';
      $('example').value = '';
      history.replaceState(null, '', '#playground');
      changed();
    });
    source.addEventListener('keydown', event => {
      if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        render();
      }
    });
    $('example').addEventListener('change', event => {
      loadExample(event.target.value);
      history.replaceState(null, '', '#playground');
    });
    $('open').addEventListener('click', () => $('file').click());
    $('file').addEventListener('change', openFile);
    $('save').addEventListener('click', () => downloadSource(source.value, name));
    $('render').addEventListener('click', render);
    $('cancel').addEventListener('click', () => {
      renderer.cancel();
      failed('Render cancelled. Edit the source or render again when ready.');
    });
    $('auto').addEventListener('change', () => {
      clearTimeout(debounce);
      if ($('auto').checked && dirty) render();
    });
    window.addEventListener('pagehide', hide);
    initialized = true;
  }

  function show(id) {
    active = true;
    if (!initialized) {
      initialize();
      const draft = readDraft();
      const demo = demos.find(item => item.id === id);
      if (demo) loadExample(id);
      else if (draft) load(draft.source, draft.name);
      else loadExample('point_relations');
      return;
    }
    if (id && id !== openedExample) loadExample(id);
    else if (dirty && $('auto').checked) render();
  }

  function hide() {
    active = false;
    clearTimeout(debounce);
    if (busy) {
      renderer.cancel();
      failed('Render paused. Return to render your source.');
    }
  }

  return { show, hide };
}
