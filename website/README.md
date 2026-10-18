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

Demo categories and introductory labels are defined in `website/build.mjs`. The current release is under active development; beta 6 status and optional PNG support are stated on the page.
