# Locus WebAssembly library

`locus-wasm` exposes the native parsing, expansion, font measurement, constraint
solving, layout, and SVG pipeline through `wasm-bindgen`. It includes real ReX
math and bundled Latin/CJK fonts; it needs no rendering server or installed fonts.

Build the library from the repository root:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --version 0.14.0 --locked
wasm-pack build crates/locus-wasm --target web --release --no-opt --out-dir ../../target/wasm/pkg --out-name locus --locked
```

The generated JavaScript module, TypeScript declarations, and WASM binary are
written to `target/wasm/pkg/`. The build skips optional Binaryen optimization;
Rust release optimization remains enabled.

From a JavaScript module served over HTTP:

```js
import init, { render } from './pkg/locus.js';

await init();
const result = JSON.parse(render(source));
if (result.ok) {
  const url = URL.createObjectURL(new Blob([result.svg], { type: 'image/svg+xml' }));
  document.querySelector('img').src = url;
  // Revoke the URL when the figure is replaced or removed.
} else {
  console.log(result.diagnostics);
}
```

`render(source)` is synchronous and returns a JSON string:

- Success: `{ ok: true, svg, diagnostics: [], warnings, assertions }`. `warnings`
  retains structured layout/inspection diagnostics, and `assertions` contains
  `passed` and individual assertion results with source spans and actual values.
  Failed assertions preserve the rendered figure, as in `locus render`.
- Failure: `{ ok: false, svg: null, diagnostics }`. Each diagnostic contains
  `code`, `message`, `span`, `notes`, and `suggestion`. Span lines/columns are
  one-based, and byte offsets are UTF-8 offsets into the submitted source.

Run it in a worker for interactive editing. Large or difficult constraint systems
can take time; terminating the worker cancels a render. After a runtime trap,
discard the instance and create a new one. Most invalid inputs produce
diagnostics, but upstream ReX panics cannot be caught with the WASM target's
default aborting panic behavior. Library timing collection uses `std::time` and
must remain disabled on this target; ordinary rendering does not read the clock.

The initial module includes the CJK font and is substantially larger than a
geometry-only module. Serve `.wasm` as `application/wasm` and enable HTTP
compression in production. SVG is the browser export; CLI I/O, PNG, TikZ, and
render-dump visualization are not exposed by this binding.

Run the Rust tests and test the generated module in Node.js 22+ (build first):

```sh
cargo test -p locus-wasm
node crates/locus-wasm/tests/smoke.mjs --all
```

The WASM checks cover all demonstrations, deterministic output, failed
assertions, CJK outlines, invalid input, TeX errors, and error recovery. Set
`LOCUS_BIN` to a native CLI binary to additionally compare the triangle SVG
byte for byte with native output.

The bundled components retain their existing terms. Include the repository's
`THIRD-PARTY-NOTICES.md`, `licenses/`, and Locus license files when distributing
the module. The [ReX provenance investigation](../../licenses/REX-LICENSING.md)
also applies to this build.
