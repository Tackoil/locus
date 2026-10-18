const keywords = new Set(`locus beta meta style class fn group assert rel where free tol and or not
point num line ray segment circle arc ellipse bspline polygon rect rounded_rect diamond label arrow
tick right_angle text_block`.split(/\s+/));

const tokens = /(?<comment>\/\/[^\r\n]*)|(?<string>"(?:\\[^\r\n]|[^"\\\r\n])*(?:"|\\?(?=\r?$)))|(?<math>\$(?:\\[^\r\n]|[^$\\\r\n])*(?:\$|\\?(?=\r?$)))|(?<color>#[\da-fA-F]{6}(?:[\da-fA-F]{2})?\b)|(?<identifier>[A-Za-z][A-Za-z\d_']*)|(?<number>(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?(?:deg|rad)?)|(?<operator><->|->|==|<=|>=|[=+*/<>-])/gm;

function escapeHTML(text) {
  return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
}

function tokenKind(match, source) {
  const word = match.groups.identifier;
  if (!word) return Object.keys(match.groups).find(kind => match.groups[kind] !== undefined);
  if (keywords.has(word)) return 'keyword';
  if (word === 'true' || word === 'false') return 'number';
  const following = source.slice(match.index + word.length);
  if (/^[ \t]*\(/.test(following)) return 'function';
  if (/^[ \t]*:/.test(following)) return 'property';
  return 'identifier';
}

export function highlightSource(source) {
  const parts = [];
  let end = 0;
  for (const match of source.matchAll(tokens)) {
    parts.push(escapeHTML(source.slice(end, match.index)));
    parts.push(`<span class="locus-${tokenKind(match, source)}">${escapeHTML(match[0])}</span>`);
    end = match.index + match[0].length;
  }
  parts.push(escapeHTML(source.slice(end)));
  return parts.join('');
}
