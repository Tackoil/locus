# ReX integration

Last tested: 2026-09-19

- upstream: <https://github.com/ReTeX/ReX>
- pinned revision: `7bd4efc0b77c6910b79668d32824b464ee0958a3`
- toolchain: Rust `1.88.0`
- result: working with the local `static_map_macros` compatibility patch

## Build compatibility

Unmodified upstream fails because `static_map_macros` 0.2.0-beta expects the
exact whitespace of an old compiler's `TokenStream` display. The local
[`vendor/static_map_macros`](../../vendor/static_map_macros/README.md) patch
extracts token groups instead. It preserves upstream's generated maps and math
layout. The optional git dependency pins ReX; Cargo.lock also pins its transitive
`fp` dependency. Keep the root Cargo patch when embedding this workspace.

## Rendering contract

`RexEngine` parses TeX and uses ReX's display-style layout, without its SVG page
padding or external font references. A custom renderer emits paths from the
bundled matching STIX Two Math font and rectangles for fraction/radical rules.
The output uses em-relative dimensions with the baseline at y = 0, positive y
pointing down. Measurements include logical layout and actual glyph/rule bounds;
overhangs do not alter the logical inline advance. Parsed font tables are cached.

The same outlines feed SVG, TikZ, and PNG. No runtime network, system fonts,
JavaScript, or external TeX installation is required. As with other Locus text,
annotation styling determines the final outline color; TeX color commands do
not override it.

Invalid sizes, raw newlines, XML-invalid source, parse errors, and missing glyphs
return typed errors. Upstream panics for some unsupported constructs are caught
at the engine boundary and reported as invalid TeX. With the standard panic hook,
those upstream panics may also print a diagnostic to stderr. CLI failures remain
source-anchored E414 errors and preserve existing output files.

ReX is a TeX subset, not full LaTeX. Tested examples include scripts, fractions,
radicals, sums, integrals, Greek letters, font styles, wide accents, stretchy
delimiters, and matrices. Unsupported commands are errors, with no mock fallback.

## Verification

```sh
cargo test -p locus-tex --features rex
cargo test -p locus-tex --no-default-features
cargo test --workspace --all-features --all-targets
cargo clippy --workspace --all-features --all-targets -- -D warnings
```
