import assert from 'node:assert/strict';
import { test } from 'node:test';
import { highlightSource } from '../src/locus-highlight.js';

test('highlights Locus declarations, calls, properties, colors, and numeric units', () => {
  const html = highlightSource('point A() where { hdist(A, B) == .5e+2 }\nlabel caption(A, $A$) { accent: #AABBCC80, rotate: 90deg }\n');
  assert.match(html, /class="locus-keyword">point<\/span>/);
  assert.match(html, /class="locus-keyword">where<\/span>/);
  assert.match(html, /class="locus-function">hdist<\/span>/);
  assert.match(html, /class="locus-property">accent<\/span>/);
  assert.match(html, /class="locus-color">#AABBCC80<\/span>/);
  assert.match(html, /class="locus-number">\.5e\+2<\/span>/);
  assert.match(html, /class="locus-number">90deg<\/span>/);
  assert.match(html, /class="locus-math">\$A\$<\/span>/);
});

test('comments and quoted strings keep keywords, URLs, and escaped quotes together', () => {
  const html = highlightSource('// point 42 "comment"\nmeta note = "https://example.test/ \\"point\\" $A$"\n');
  assert.match(html, /class="locus-comment">\/\/ point 42 &quot;comment&quot;<\/span>/);
  assert.match(html, /class="locus-string">&quot;https:\/\/example\.test\/ \\&quot;point\\&quot; \$A\$&quot;<\/span>/);
  assert.equal((html.match(/locus-keyword/g) || []).length, 1);
});

test('unfinished strings and TeX stay highlighted while typing without consuming the next line', () => {
  const html = highlightSource('"unfinished\\\npoint A()\n$\\alpha + 1\nassert true\n');
  assert.match(html, /class="locus-string">&quot;unfinished\\<\/span>\n/);
  assert.match(html, /class="locus-math">\$\\alpha \+ 1<\/span>\n/);
  assert.match(html, /class="locus-keyword">assert<\/span>/);
});

test('escapes HTML and preserves whitespace, Unicode, identifiers, and final empty lines', () => {
  const source = '\tlabel 中文(A, "<img src=x onerror=alert(1)> & 中文 😀")\npoint1 A_prime\'\n\n';
  const html = highlightSource(source);
  assert.doesNotMatch(html, /<img/);
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt; &amp; 中文 😀/);
  assert.match(html, /class="locus-identifier">point1<\/span>/);
  const plain = html.replace(/<[^>]+>/g, '').replaceAll('&quot;', '"').replaceAll('&#39;', "'").replaceAll('&lt;', '<').replaceAll('&gt;', '>').replaceAll('&amp;', '&');
  assert.equal(plain, source);
  assert.equal(highlightSource(''), '');
  assert.equal(highlightSource(source), html);
});
