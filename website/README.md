# Locus project website

A static introduction, gallery of all 52 canonical demonstrations, browser playground, and searchable documentation reader. The gallery contains native Locus SVG output and downloadable original source. Documentation is generated from the repository, including the complete specification.

## Build and preview

From the repository root:

```sh
cargo build --release -p locus-cli --locked
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --version 0.14.0 --locked
npm ci --prefix website
npm test --prefix website
npm run build --prefix website
npm run preview --prefix website
```

Open <http://127.0.0.1:4173/#playground>. Serve the generated `website/dist/` directory on any static host. There is no application server. The page uses optional Google Fonts, with system-font fallbacks. Serve `.wasm` as `application/wasm` and enable HTTP compression for the approximately 21 MB module.

The build first creates the release WASM package using `wasm-pack`, then copies the JavaScript binding, binary, and required license notices into `dist/`. Run `npm run build:wasm --prefix website` to rebuild only WASM, or `node website/build.mjs` to package an already-built WASM library and rebuild the site.

## GitHub Pages

In the repository’s **Settings → Pages → Build and deployment**, set **Source** to **GitHub Actions**. This is the one-time setup required by [GitHub Pages custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

The [`pages.yml`](../.github/workflows/pages.yml) workflow builds and publishes the website on every push to `main`. After pushing the workflow, you can also run **Deploy website to GitHub Pages** manually from the Actions tab with `main` selected. Manual runs on other branches build and test the site without publishing it.

The workflow installs the pinned Rust toolchain’s WASM target, wasm-pack, and Node.js dependencies; builds the native renderer and release WASM module; runs the website tests and all WASM examples; and uploads `website/dist/` to the `github-pages` deployment environment. Only the deployment job receives permission to publish. A failed build or test prevents publication.

For this repository, the default project URL is <https://tackoil.github.io/locus/> and the playground is <https://tackoil.github.io/locus/#playground>. The workflow reports the actual URL in its deployment environment. These links become available after the first successful deployment. Assets, fonts, documentation, worker modules, and WASM URLs are relative to the site directory, so the same build supports a project subdirectory or a custom domain without a base-path setting. Hash routes can be opened and reloaded directly without server rewrite rules.

## Playground

Open `#playground` from the navigation or homepage, or choose **Edit in playground** in a demo dialog (`#playground/<demo-id>`). Start with the triangle, load any canonical demo, or open a local `.locus` file. Edit in the source panel, toggle automatic rendering, or render with Ctrl/Command + Enter. Download the current source or the latest successful SVG. Editing invalidates the SVG download until the source is rendered again.

The full Locus pipeline runs locally in a module worker, loaded only when the playground opens. Source never goes to a rendering server. Cancel terminates the worker, as does a 30-second timeout; another render starts a fresh instance. Runtime traps and loading failures offer a retry. Source errors retain the last successful preview with an explicit stale-preview caption. Failed assertions keep the SVG available and appear beside warnings and source-linked diagnostics.

The source editor highlights Locus keywords, functions, strings, TeX, comments, numbers, colors, and properties. Its line-number gutter follows vertical scrolling while the highlighted source follows both axes. Highlighting updates as you type, load examples, or open files, and preserves native selection, copy/paste, undo, resizing, and diagnostic navigation. The decorative layers are hidden from assistive technology; forced-color mode uses native text colors.

The current draft is saved in local storage when available and survives navigation/reloads. Choosing an example or opening a file replaces that draft; save `.locus` first to keep another copy. Browser storage restrictions do not prevent editing or rendering. The playground stacks its source and preview panels on smaller screens and follows the website’s light/dark theme.

`npm test --prefix website` checks syntax highlighting (including incomplete input, escaping, and source preservation), cancellation, stale responses, retry after errors/timeouts, diagnostic formatting, and draft storage without a browser. The CI WASM job also builds the website and verifies the full WASM demonstration suite.

The build runs native checks, executes assertions, and renders every `.locus` file in `examples/demonstrations/`. A failed check stops the build. Checks with non-fatal warnings are identified in the demo viewer. Render results are cached against source contents and the CLI binary’s size and modification time. Delete `website/.cache/` to force revalidation. Set `LOCUS_BIN` to use another CLI executable.

Edit presentation files in `website/src/`. The build copies them into `dist/`, packages documentation from the repository, and generates `content.js`. No repository documentation or canonical examples are modified.

The hero uses `euler_nine_point_showcase.locus`. Its presentation colors are adapted and its poster captions omitted by `hero-theme.mjs` before native validation and rendering; the geometry and assertions remain unchanged. The transparent SVG sits on the dotted drafting background and follows the same light/dark treatment as the other figures.

Demo categories and introductory labels are defined in `website/build.mjs`. The current release is under active development; beta 6 status and optional PNG support are stated on the page.

The homepage previews six demos on desktop, four at widths up to 800px, and three at widths up to 580px. The separate `#examples` page holds the complete searchable, filterable collection. Demo dialogs return to the page they were opened from.

The homepage selection is pinned in `src/app.js`: ellipse foci, Sankey energy flow, the agent loop, system architecture, checkout sequence, and Monge’s theorem. The order gives phones geometry, data, and workflow examples; tablets also show systems. The full collection keeps its own ordering.

Display headings and the hero slogan use a self-hosted Latin subset of Roboto Serif with its variable width, weight, and optical-size axes. The hero heading uses an extended width. Roboto Serif is distributed under the SIL Open Font License 1.1; the copyright notice and complete license are included in `src/fonts/RobotoSerif-OFL.txt` and copied into the published assets. Provenance is recorded in `src/fonts/README.txt`. Small headings and interface text use the existing sans-serif stack.
