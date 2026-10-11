# ReX licensing investigation

Investigated on 2026-10-15. Locus retains MIT OR Apache-2.0 for its own code.
This record distinguishes upstream statements from unresolved provenance;
adding notices does not grant rights absent from the original sources.

## Pinned source and current use

Locus pins [ReTeX/ReX commit
7bd4efc0b77c6910b79668d32824b464ee0958a3](https://github.com/ReTeX/ReX/tree/7bd4efc0b77c6910b79668d32824b464ee0958a3),
dated 2020-07-15. The packages `rex`, `font_types`, and `stix2` at this revision
are all version 0.1.0. None declares `license` or `license-file` in its Cargo
manifest. The root manifest names Christopher Breeden and Sebastian Köln;
the font packages name Christopher Breeden.

The `rex` feature is optional in `locus-tex`, but
[`locus-cli`](../crates/locus-cli/Cargo.toml) explicitly enables that feature in
its dependency declaration. A normal CLI build therefore includes ReX.
Selecting the mock engine at runtime does not remove that dependency.

## Declared licensing and missing files

The pinned [README](third-party/rex-README.txt) states:

> ReX is primarily distributed under the terms of both the MIT license and
> the Apache License (Version 2.0), with portions covered by various BSD-like
> licenses.
>
> See LICENSE-APACHE, and LICENSE-MIT for details.

The statement was introduced by [commit
ed07d577fd66b59a9ceb4736a42c6e76ba09db50](https://github.com/ReTeX/ReX/commit/ed07d577fd66b59a9ceb4736a42c6e76ba09db50)
on 2016-12-17. That commit changes only the README; it does not add license
files. The pinned tree contains neither referenced file, and searching all
389 reachable commits found no license, COPYING, or NOTICE file history.
GitHub's contents listing for the pinned revision confirmed their absence;
its `LICENSE`, `LICENSE-MIT`, and `LICENSE-APACHE` commit-history queries were
also empty. These observations establish missing documentation, not an
absence of every possible licensing grant.

The README is preserved verbatim as evidence. Locus has not manufactured a
ReX-specific MIT copyright notice, filled in unnamed BSD licenses, or assigned
a definitive SPDX expression to these three packages.

## Borrowed macros

[`src/macros.rs`](https://github.com/ReTeX/ReX/blob/7bd4efc0b77c6910b79668d32824b464ee0958a3/src/macros.rs)
attributes its `min!` and `max!` definitions to Emanuel Claesson's
`max_min_macros` crate. The [attribution and definitions](third-party/rex-borrowed-macros.txt)
are preserved verbatim. The exact original package revision and its license
were not recovered. This attribution alone does not identify the BSD-like
portions mentioned in the README.

## Generated symbol and font tables

The [header of `fonts/stix2/src/symbols.rs`](third-party/rex-symbols-provenance.txt)
explicitly says the file was generated from `unicode-math-table.tex` in CTAN's
`unicode-math` package. `fonts/stix2/src/glyphs.rs` repeats that source header.
The pinned ReX tree does not include the cited generator or original input.

The historical `unicode-math` v0.8g source, at
[commit ec953b2bf6b2584e66277291092fce1704b06e7b](https://github.com/latex3/unicode-math/tree/ec953b2bf6b2584e66277291092fce1704b06e7b),
licenses its package under LPPL-1.3c or later and explicitly lists
`unicode-math-table.tex` as part of the work. Its
[README and author notices](third-party/unicode-math-README-v0.8g.txt) and
[LPPL text](third-party/unicode-math-LPPL-1.3c.txt) are preserved as historical
provenance evidence. [CTAN](https://ctan.org/pkg/unicode-math) also identifies
the current package as LPPL-1.3c.

This does not prove which input revision ReX used, whether all generated data
is copyrightable, or which LPPL conditions apply to the generated Rust tables.
It does mean the README's permissive-license summary alone is insufficient
to clear those tables. The supplemental LPPL text is evidence, not a conclusion
that every ReX file is LPPL-licensed or that its requirements have been met.

The bundled STIX Two Math font separately retains its
[OFL-1.1 terms](../crates/locus-tex/assets/STIX-OFL.txt). Its pinned origin and
checksum are recorded in the [font README](../crates/locus-tex/assets/README.md).
The source and applicable terms of generated font metrics and glyph tables
also need to be confirmed; the font's notice alone does not settle all table
provenance questions.

## What remains to establish before release

1. Recover authoritative ReX MIT/Apache license texts and applicable copyright
   notices, or obtain upstream clarification covering the pinned revision and
   its internal font packages.
2. Identify the README's BSD-like portions and preserve their exact notices;
   confirm the source and license of the attributed macros.
3. Locate the generator and precise `unicode-math` input used for the tables;
   establish and satisfy the applicable conditions, or replace the tables
   with data whose provenance and redistribution terms are established.
4. Confirm the applicable terms and notices for font-derived tables, then
   update the supplemental records and regenerate the third-party notices.

Until those items are resolved, this repository records an outstanding ReX
licensing review; it does not claim that notice collection clears a ReX-based
release. Changing Locus's own license would not resolve these source issues.
No upstream maintainers were contacted during this investigation.

## Reproducing the evidence

The generated [third-party notices](../THIRD-PARTY-NOTICES.md) preserve the
available source declarations and attributions. Supplemental texts have origin
URLs and SHA-256 hashes in [sources.json](third-party/sources.json).
Hashes are checked during notice generation. Upstream files recovered from
source archives use immutable commit URLs; the `unicode-math` archive's commit
was read from its Git archive metadata. That version is historical evidence,
not an assertion that ReX used precisely v0.8g.

The local Git checks were `git ls-tree -r`, `git log --all --name-only`,
`git show ed07d577fd66b59a9ceb4736a42c6e76ba09db50`, and
`git rev-list --count HEAD` in the pinned ReX checkout. Findings describe the
pinned revision and reachable history, not every fork or private agreement.
