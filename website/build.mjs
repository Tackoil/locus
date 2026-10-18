import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { cp, mkdir, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { marked, Renderer } from 'marked';
import { themeHeroSource } from './hero-theme.mjs';

const run = promisify(execFile);
const site = path.dirname(fileURLToPath(import.meta.url));
const root = path.dirname(site);
const dist = path.join(site, 'dist');
const binary = process.env.LOCUS_BIN || path.join(root, 'target/release/locus');
const repository = 'https://github.com/Tackoil/locus';
const geometry = new Set(['aec3_filter_alignment', 'angle_bisector_ratio', 'bspline_controls', 'ellipse_foci', 'euler_nine_point', 'inscribed_angle', 'monge_theorem', 'pentagon_golden', 'point_relations', 'rounded_rect', 'sequence_check', 'tangent_circles']);
const workflows = new Set(['flowchart_agent_loop', 'checkout_sequence', 'swimlanes', 'state_diagram', 'user_journey', 'gantt_chart', 'git_graph', 'kanban_board', 'timeline', 'zenuml']);
const charts = new Set(['pie_chart', 'quadrant_chart', 'sankey', 'xy_chart', 'radar_chart', 'treemap', 'venn_diagram', 'cynefin', 'wardley_map']);
const featured = ['point_relations', 'flowchart_agent_loop', 'checkout_sequence', 'sankey', 'euler_nine_point', 'architecture'];
const names = { point_relations: 'A triangle, by relation', flowchart_agent_loop: 'The agent loop', checkout_sequence: 'A checkout conversation', sankey: 'Follow the energy', euler_nine_point: 'Euler’s nine-point circle', monge_theorem: 'Monge’s theorem', architecture: 'A system, connected', aec3_filter_alignment: 'Filter-bank alignment', bspline_controls: 'B-spline controls', c4_container: 'C4 containers', entity_relationship: 'Entity relationships', xy_chart: 'Sales over time' };

const documents = [
  { id: 'getting-started', title: 'Getting started', subtitle: 'From source to your first verified figure.', file: 'website/src/getting-started.md', group: 'Start here' },
  { id: 'language', title: 'Authoring reference', subtitle: 'Syntax, geometry, relations, and reusable functions.', file: 'skills/locus/references/language.md', group: 'The language' },
  { id: 'specification', title: 'Language specification', subtitle: 'The complete, normative beta 6 contract.', file: 'spec/specification.md', group: 'The language' },
  { id: 'project', title: 'Project & CLI guide', subtitle: 'Rendering, inspection, diagnostics, and development.', file: 'README.md', group: 'The toolchain' },
  { id: 'examples', title: 'Demonstration guide', subtitle: 'All 43 examples, their scope, and validation.', file: 'examples/demonstrations/README.md', group: 'The toolchain' },
  { id: 'agent', title: 'Working with agents', subtitle: 'The project’s authoring and verification workflow.', file: 'skills/locus/SKILL.md', group: 'The toolchain' },
  { id: 'editor', title: 'Zed editor integration', subtitle: 'Highlighting, completion, and language-server setup.', file: 'ecosystem/zed/README.md', group: 'The toolchain' },
  { id: 'typography', title: 'Text & mathematics', subtitle: 'Bundled fonts, TeX support, and rendering details.', file: 'crates/locus-tex/README.md', group: 'The toolchain' },
];

function category(id) {
  if (geometry.has(id)) return 'Geometry';
  if (workflows.has(id)) return 'Workflows';
  if (charts.has(id)) return 'Data';
  return 'Systems';
}

function displayName(id) {
  return names[id] || id.replaceAll('_', ' ').replace(/^./, letter => letter.toUpperCase());
}

async function readCache(file) {
  try { return JSON.parse(await readFile(file, 'utf8')); }
  catch { return null; }
}

async function renderDemo(file, binaryKey) {
  const id = path.basename(file, '.locus');
  let input = path.join(root, 'examples/demonstrations', file);
  let source = await readFile(input, 'utf8');
  if (id === 'euler_nine_point_showcase') {
    source = themeHeroSource(source);
    input = path.join(site, '.cache', file);
    await writeFile(input, source);
  }
  const key = createHash('sha256').update(source + binaryKey).digest('hex');
  const cacheFile = path.join(site, '.cache', `${id}.json`);
  const cache = await readCache(cacheFile);
  const output = path.join(dist, 'demos', `${id}.svg`);
  let check = cache?.check;
  if (cache?.key !== key || !(await stat(output).catch(() => null))) {
    const checked = await run(binary, ['check', input, '--json', '--compact'], { cwd: root, timeout: 240000, maxBuffer: 8 * 1024 * 1024 });
    check = JSON.parse(checked.stdout).files[0];
    if (!check.passed) throw new Error(`Native validation failed: ${file}`);
    await run(binary, ['render', input, '-o', output], { cwd: root, timeout: 240000, maxBuffer: 8 * 1024 * 1024 });
    await writeFile(cacheFile, JSON.stringify({ key, check }));
  }
  await writeFile(path.join(dist, 'demos', file), source);
  const title = source.match(/meta title = "([^"]+)"/)?.[1] || displayName(id);
  const description = source.match(/meta note = "([^"]+)"/)?.[1] || title;
  console.log(`Verified ${id}`);
  const warnings = check.diagnostics.filter(item => item.severity === 'warning').map(item => item.message);
  return { id, name: displayName(id), title, description, category: category(id), source, assertions: check.assertions.total, warnings };
}

function slug(text) {
  return text.toLowerCase().replace(/<[^>]*>/g, '').replace(/[^\p{L}\p{N}\s_-]/gu, '').trim().replace(/\s+/g, '-');
}

function rewriteLink(href, document) {
  if (!href || /^(https?:|mailto:)/.test(href)) return href;
  const [relative, anchor] = href.split('#');
  const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(document.file), relative));
  const target = relative ? documents.find(item => item.file === resolved) : document;
  if (target) return `#docs/${target.id}${anchor ? '/' + anchor : ''}`;
  if (resolved.startsWith('examples/demonstrations/') && resolved.endsWith('.locus')) return `#demo/${path.posix.basename(resolved, '.locus')}`;
  return `${repository}/blob/HEAD/${resolved}${anchor ? '#' + anchor : ''}`;
}

async function renderDocument(document) {
  let source = await readFile(path.join(root, document.file), 'utf8');
  source = source.replace(/^---\n[\s\S]*?\n---\n/, '');
  const headings = [];
  const used = new Map();
  const renderer = new Renderer();
  renderer.heading = function ({ tokens, depth }) {
    const title = this.parser.parseInline(tokens);
    const base = slug(tokens.map(token => token.raw).join(''));
    const count = used.get(base) || 0;
    used.set(base, count + 1);
    const id = count ? `${base}-${count}` : base;
    headings.push({ title: title.replace(/<[^>]*>/g, ''), id, depth });
    return `<h${depth} id="doc-${id}">${title}</h${depth}>\n`;
  };
  const html = marked.parse(source, {
    gfm: true, renderer,
    walkTokens(token) {
      if (token.type === 'link') token.href = rewriteLink(token.href, document);
      if (token.type === 'image') token.href = 'assets/locus-banner.svg';
    },
  });
  await writeFile(path.join(dist, 'docs', `${document.id}.md`), source);
  return { ...document, html, headings };
}

async function main() {
  await Promise.all(['demos', 'docs', 'assets'].map(directory => mkdir(path.join(dist, directory), { recursive: true })));
  await mkdir(path.join(site, '.cache'), { recursive: true });
  await cp(path.join(site, 'src'), dist, { recursive: true, filter: file => !file.endsWith('.md') });
  await Promise.all(['locus-mark.svg', 'locus-banner.svg'].map(file => cp(path.join(root, 'assets', file), path.join(dist, 'assets', file))));
  const binaryInfo = await stat(binary);
  const files = (await readdir(path.join(root, 'examples/demonstrations'))).filter(file => file.endsWith('.locus'));
  const queue = [...files];
  const demos = [];
  const worker = async () => {
    while (queue.length) demos.push(await renderDemo(queue.shift(), `${binaryInfo.size}:${binaryInfo.mtimeMs}`));
  };
  await Promise.all(Array.from({ length: 3 }, worker));
  demos.sort((a, b) => {
    const order = id => featured.includes(id) ? featured.indexOf(id) : 100;
    return order(a.id) - order(b.id) || a.name.localeCompare(b.name);
  });
  const docs = await Promise.all(documents.map(renderDocument));
  await writeFile(path.join(dist, 'content.js'), `window.LOCUS_DATA = ${JSON.stringify({ demos, docs })};\n`);
  console.log(`Built ${demos.length} verified demos and ${docs.length} documentation pages in ${dist}`);
}

await main();
