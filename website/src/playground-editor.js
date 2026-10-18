import { highlightSource } from './locus-highlight.js';

function decoration(className) {
  const pre = document.createElement('pre');
  pre.className = className;
  pre.setAttribute('aria-hidden', 'true');
  const content = document.createElement('code');
  pre.append(content);
  return { pre, content };
}

export function createCodeEditor(source) {
  const editor = document.createElement('div');
  editor.className = 'code-editor';
  const viewport = document.createElement('div');
  viewport.className = 'code-editor-viewport';
  const gutter = decoration('code-editor-gutter');
  const highlight = decoration('code-editor-highlight');
  source.before(editor);
  editor.append(gutter.pre, viewport);
  viewport.append(highlight.pre, source);

  function syncScroll() {
    highlight.pre.scrollTop = source.scrollTop;
    highlight.pre.scrollLeft = source.scrollLeft;
    gutter.pre.scrollTop = source.scrollTop;
  }

  function resize() {
    highlight.pre.style.width = `${source.clientWidth}px`;
    highlight.pre.style.height = `${source.clientHeight}px`;
    gutter.pre.style.height = `${source.clientHeight}px`;
    syncScroll();
  }

  function update() {
    const count = source.value.split('\n').length;
    // The sentinel keeps a trailing empty line aligned with the native caret.
    highlight.content.innerHTML = highlightSource(source.value) + '\u200b';
    gutter.content.textContent = Array.from({ length: count }, (_, index) => index + 1).join('\n');
    editor.style.setProperty('--line-digits', Math.max(2, String(count).length));
    resize();
    return count;
  }

  source.addEventListener('scroll', syncScroll, { passive: true });
  const observer = new ResizeObserver(resize);
  observer.observe(source);
  update();
  return { update, syncScroll };
}
