# Locus project website

A static introduction, gallery of all 43 canonical demonstrations, and searchable documentation reader. The gallery contains native Locus SVG output and downloadable original source. Documentation is generated from the repository, including the complete specification.

## Build and preview

From the repository root:

```sh
cargo build --release -p locus-cli --locked
npm ci --prefix website
npm run build --prefix website
npm run preview --prefix website
```

Open <http://127.0.0.1:4173>. Serve the generated `website/dist/` directory on any static host. There is no browser-side package dependency or application server. The page uses optional Google Fonts, with system-font fallbacks.

The build runs native checks, executes assertions, and renders every `.locus` file in `examples/demonstrations/`. A failed check stops the build. Checks with non-fatal warnings are identified in the demo viewer. Render results are cached against source contents and the CLI binary’s size and modification time. Delete `website/.cache/` to force revalidation. Set `LOCUS_BIN` to use another CLI executable.

Edit presentation files in `website/src/`. The build copies them into `dist/`, packages documentation from the repository, and generates `content.js`. No repository documentation or canonical examples are modified.

The hero uses `euler_nine_point_showcase.locus`. Its presentation colors are adapted and its poster captions omitted by `hero-theme.mjs` before native validation and rendering; the geometry and assertions remain unchanged. The transparent SVG sits on the dotted drafting background and follows the same light/dark treatment as the other figures.

Demo categories and introductory labels are defined in `website/build.mjs`. The current release is under active development; beta 6 status and optional PNG support are stated on the page.

The homepage previews six demos on desktop, four at widths up to 800px, and three at widths up to 580px. The separate `#examples` page holds the complete searchable, filterable collection. Demo dialogs return to the page they were opened from.

The homepage selection is pinned in `src/app.js`: ellipse foci, Sankey energy flow, the agent loop, system architecture, checkout sequence, and Monge’s theorem. The order gives phones geometry, data, and workflow examples; tablets also show systems. The full collection keeps its own ordering.

Display headings and the hero slogan use a self-hosted Latin subset of Roboto Serif with its variable width, weight, and optical-size axes. The hero heading uses an extended width. Roboto Serif is distributed under the SIL Open Font License 1.1; the copyright notice and complete license are included in `src/fonts/RobotoSerif-OFL.txt` and copied into the published assets. Provenance is recorded in `src/fonts/README.txt`. Small headings and interface text use the existing sans-serif stack.
