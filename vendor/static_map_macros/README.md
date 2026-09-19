# static_map_macros compatibility patch

Vendored from `static_map_macros` 0.2.0-beta, by Christopher Breeden:
<https://github.com/cbreeden/static-map>. Upstream declares MIT/Apache-2.0 dual
licensing in its package manifest; both license texts are included here.

The original derive macro extracts its input by matching the exact whitespace
in `TokenStream::to_string()`. That format changed in modern Rust, causing every
ReX/STIX2 map expansion to panic before the backend can compile.

Local changes:

- Traverse the enum and macro invocation token groups, then skip the `@zero`
  prefix. This preserves the original map generator without relying on rustc's
  display formatting. String literals and their whitespace remain intact.
- Use a literal format string in the builder's panic diagnostic.
- Keep only the library and its dependencies in the manifest.

The root `[patch.crates-io]` applies this to ReX and its font tables. This is an
excluded third-party package, not a Locus workspace member. Building and testing
`locus-tex --features rex` exercises integer glyph maps and string command maps.
