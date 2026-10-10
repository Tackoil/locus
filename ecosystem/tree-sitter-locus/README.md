# Tree-sitter Locus

An editor grammar for the current Locus `0 beta 5` format. It covers declarations, metadata,
styles, `where` clauses, attributes, properties, expressions, references,
primed identifiers, numeric units, strings, and bare/inline TeX. Comments and
escape sequences inside text are distinguished from source-level comments.

The Rust parser remains authoritative for version rules, constructor names and
arity, types, reference order, and other diagnostics. This grammar deliberately
accepts unfinished files without a final newline and does not require a header
before highlighting, so it remains useful while typing. It preserves physical
line boundaries and recovers after malformed calls and unterminated text.

## Develop and test

```sh
npm ci
npm run generate
npm test
```

Commit `grammar.js`, the generated `src/` files, and the lockfile together. The
CLI is pinned to 0.25.10 and the generated parser uses ABI 14 for compatibility.
`npm test` checks parse-tree fixtures, highlighting assertions, every source in
`spec/test` and `assets`, all four Zed query files, newline variants, and recovery
after malformed input. The query files live in `../zed/languages/locus` so the
tests exercise exactly the rules shipped to Zed.

## Use with Zed

Follow the [Zed extension setup](../zed/README.md). The preparation script stages
a local grammar snapshot for Zed; no published grammar repository is required.

If publishing an extension later, host the grammar in a Git repository and use
its URL and an immutable commit in `[grammars.locus]`. If hosted in the main
Locus repository, also set `path = "ecosystem/tree-sitter-locus"` in that entry.
