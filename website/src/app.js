const { demos, docs } = window.LOCUS_DATA;
const $ = selector => document.querySelector(selector);
const filters = [...document.querySelectorAll('[data-filter]')];
const mobileViewport = window.matchMedia('(max-width: 580px)');
const tabletViewport = window.matchMedia('(max-width: 800px)');
const featuredDemos = [
  'ellipse_foci',
  'sankey',
  'flowchart_agent_loop',
  'architecture',
  'checkout_sequence',
  'monge_theorem',
].map(id => demos.find(demo => demo.id === id)).filter(Boolean);
const state = { category: 'All', search: '', selected: null, returnHash: '#demos', doc: null, view: 'home' };
const dialog = $('#demo-dialog');
let toastTimer;

function escapeHTML(value) {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
}

function highlight(source) {
  const expression = /(\/\/[^\n]*|"(?:[^"\\]|\\.)*"|\$[^$\n]*\$|\b(?:locus|style|meta|class|fn|group|point|circle|polygon|rect|rounded_rect|diamond|arrow|label|segment|line|ray|num|ellipse|bspline|arc|right_angle|assert|rel|where|and|or|not)\b|\b\d+(?:\.\d+)?\b)/g;
  return source.split(expression).map((part, index) => {
    if (index % 2 === 0) return escapeHTML(part);
    return `<span class="token-${tokenKind(part)}">${escapeHTML(part)}</span>`;
  }).join('');
}

function tokenKind(part) {
  if (part.startsWith('//')) return 'comment';
  if (/^["$]/.test(part)) return 'string';
  if (/^\d/.test(part)) return 'number';
  return 'keyword';
}

function card(demo) {
  return `<article class="demo-card"><button type="button" data-demo="${demo.id}" aria-label="Explore ${escapeHTML(demo.name)}"><div class="demo-card-preview"><img src="demos/${demo.id}.svg" alt="${escapeHTML(demo.title)}" loading="lazy" width="380" height="220"></div><div class="demo-card-info"><div><h3>${escapeHTML(demo.name)}</h3><span class="demo-category">${demo.category}</span></div><p><span>${demo.id}.locus</span><span>View source ↗</span></p></div></button></article>`;
}

function matchesDemo(demo) {
  const categoryMatches = state.category === 'All' || demo.category === state.category;
  return categoryMatches && `${demo.name} ${demo.title} ${demo.id} ${demo.description}`.toLowerCase().includes(state.search);
}

function renderFeaturedGallery() {
  const count = mobileViewport.matches ? 3 : tabletViewport.matches ? 4 : 6;
  const featured = featuredDemos.slice(0, count);
  $('#featured-demo-grid').innerHTML = featured.map(card).join('');
  $('#featured-gallery-count').textContent = `${String(featured.length).padStart(2, '0')} / ${demos.length} EXAMPLES`;
}

function renderGallery() {
  const matches = demos.filter(matchesDemo);
  $('#demo-grid').innerHTML = matches.map(card).join('') || '<p class="gallery-empty">No examples found. Try another search or category.</p>';
  $('#gallery-count').textContent = `${String(matches.length).padStart(2, '0')} / ${demos.length} EXAMPLES`;
  $('#gallery-status').textContent = `${matches.length} demonstrations found.`;
  filters.forEach(button => {
    const active = button.dataset.filter === state.category;
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', active);
  });
}

function selectDemo(event) {
  const button = event.target.closest('[data-demo]');
  if (button) location.hash = `demo/${button.dataset.demo}`;
}

function openDemo(id) {
  const demo = demos.find(item => item.id === id);
  if (!demo) return showToast('This demonstration could not be found.');
  state.selected = demo;
  $('#demo-dialog-title').textContent = demo.name;
  $('#dialog-category').textContent = `${demo.category} / LOCUS 0 BETA 6`;
  $('#dialog-description').textContent = demo.description;
  $('#dialog-validation').textContent = validationText(demo);
  $('#dialog-image').src = `demos/${demo.id}.svg`;
  $('#dialog-image').alt = demo.title;
  $('#dialog-filename').textContent = `${demo.id}.locus`;
  $('#dialog-code').innerHTML = highlight(demo.source);
  $('#download-locus').href = `demos/${demo.id}.locus`;
  $('#download-svg').href = `demos/${demo.id}.svg`;
  if (!dialog.open) dialog.showModal();
  document.body.classList.add('dialog-open');
}

function validationText(demo) {
  const checked = demo.assertions ? `✓ ${demo.assertions} assertions passed` : '✓ Native check passed';
  return demo.warnings.length ? `${checked} · ${demo.warnings.length} warning(s)` : checked;
}

function closeDemo() {
  dialog.close();
  document.body.classList.remove('dialog-open');
  if (location.hash.startsWith('#demo/')) location.hash = state.returnHash;
}

function documentNavigation() {
  let previousGroup = '';
  $('#docs-nav').innerHTML = docs.map(doc => {
    const group = doc.group !== previousGroup ? `<p class="docs-nav-group">${doc.group.toUpperCase()}</p>` : '';
    previousGroup = doc.group;
    return `${group}<a href="#docs/${doc.id}" data-doc="${doc.id}">${doc.title}</a>`;
  }).join('');
}

function showDocument(id, heading) {
  const doc = docs.find(item => item.id === id) || docs[0];
  const changed = state.doc !== doc.id;
  state.doc = doc.id;
  state.view = 'docs';
  $('#home-view').hidden = true;
  $('#examples-view').hidden = true;
  $('#docs-view').hidden = false;
  $('#document-content').innerHTML = doc.html;
  $('#document-breadcrumb').textContent = `${doc.group} / ${doc.title}`;
  $('#document-source').href = `https://github.com/Tackoil/locus/blob/HEAD/${doc.file}`;
  if (doc.id === 'getting-started') $('#document-source').href = 'docs/getting-started.md';
  $('#document-download').href = `docs/${doc.id}.md`;
  $('#document-toc').innerHTML = doc.headings.filter(item => item.depth === 2).map(item => `<a href="#docs/${doc.id}/${item.id}">${escapeHTML(item.title)}</a>`).join('');
  document.querySelectorAll('[data-doc]').forEach(link => {
    const active = link.dataset.doc === doc.id;
    link.classList.toggle('active', active);
    if (active) link.setAttribute('aria-current', 'page');
    else link.removeAttribute('aria-current');
  });
  document.title = `${doc.title} — Locus`;
  if (heading) requestAnimationFrame(() => document.getElementById(`doc-${heading}`)?.scrollIntoView());
  else if (changed) window.scrollTo({ top: 0, behavior: 'instant' });
}

function showCollection() {
  const changed = state.view !== 'examples';
  state.view = 'examples';
  state.doc = null;
  $('#home-view').hidden = true;
  $('#docs-view').hidden = true;
  $('#examples-view').hidden = false;
  document.title = 'Examples — Locus';
  if (changed) window.scrollTo({ top: 0, behavior: 'instant' });
}

function route() {
  const hash = location.hash;
  const [, section, id, heading] = hash.match(/^#([^/]+)\/?([^/]*)\/?(.*)$/) || [];
  if (section === 'demo') {
    openDemo(id);
    return;
  }
  if (dialog.open) closeDemo();
  state.returnHash = hash || '#';
  if (section === 'docs') {
    showDocument(id, heading);
    return;
  }
  if (section === 'examples') {
    showCollection();
    return;
  }
  state.doc = null;
  state.view = 'home';
  $('#home-view').hidden = false;
  $('#docs-view').hidden = true;
  $('#examples-view').hidden = true;
  document.title = 'Locus — Geometry, declared.';
  if (hash) requestAnimationFrame(() => document.getElementById(hash.slice(1))?.scrollIntoView());
  else window.scrollTo({ top: 0, behavior: 'instant' });
}

function searchDocuments(value) {
  const query = value.trim().toLowerCase();
  $('#docs-nav').hidden = !!query;
  $('#doc-search-results').hidden = !query;
  if (!query) return;
  const results = docs.filter(doc => `${doc.title} ${doc.subtitle} ${doc.headings.map(item => item.title).join(' ')} ${doc.html.replace(/<[^>]*>/g, ' ')}`.toLowerCase().includes(query));
  $('#doc-search-results').innerHTML = results.map(doc => `<a href="#docs/${doc.id}">${doc.title}<small>${doc.subtitle}</small></a>`).join('') || '<p>No documentation found. Try “relations”, “render”, or “assert”.</p>';
}

function showToast(message) {
  clearTimeout(toastTimer);
  $('#toast').textContent = message;
  $('#toast').hidden = false;
  toastTimer = setTimeout(() => { $('#toast').hidden = true; }, 2600);
}

async function copySource() {
  try {
    await navigator.clipboard.writeText(state.selected.source);
    showToast('Locus source copied.');
  } catch {
    const range = document.createRange();
    range.selectNodeContents($('#dialog-code'));
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    showToast('Source selected. Use your keyboard shortcut to copy.');
  }
}

filters.forEach(button => button.addEventListener('click', () => {
  state.category = button.dataset.filter;
  renderGallery();
}));
$('#demo-search').addEventListener('input', event => {
  state.search = event.target.value.trim().toLowerCase();
  renderGallery();
});
$('#featured-demo-grid').addEventListener('click', selectDemo);
$('#demo-grid').addEventListener('click', selectDemo);
mobileViewport.addEventListener('change', renderFeaturedGallery);
tabletViewport.addEventListener('change', renderFeaturedGallery);
$('#close-dialog').addEventListener('click', closeDemo);
dialog.addEventListener('cancel', event => { event.preventDefault(); closeDemo(); });
dialog.addEventListener('click', event => { if (event.target === dialog) closeDemo(); });
$('#copy-source').addEventListener('click', copySource);
$('#docs-search').addEventListener('input', event => searchDocuments(event.target.value));
$('#doc-search-results').addEventListener('click', event => {
  if (!event.target.closest('a')) return;
  $('#docs-search').value = '';
  searchDocuments('');
});
window.addEventListener('hashchange', route);
$('.skip-link').addEventListener('click', event => {
  event.preventDefault();
  $('#main').focus();
  $('#main').scrollIntoView();
});
documentNavigation();
document.querySelectorAll('[data-demo-total]').forEach(element => { element.textContent = demos.length; });
renderFeaturedGallery();
renderGallery();
route();
