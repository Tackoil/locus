# Bundled fonts

`NotoSans-Regular.ttf` is the deterministic default face used by
`PlaintextEngine`. It was downloaded from the official
[`notofonts/noto-fonts`](https://github.com/notofonts/noto-fonts) repository:

- upstream path: `hinted/ttf/NotoSans/NotoSans-Regular.ttf`
- SHA-256: `b85c38ecea8a7cfb39c24e395a4007474fa5a4fc864f6ee33309eb4948d232d5`
- license: SIL Open Font License 1.1, reproduced in `OFL.txt`

The font is rendered as SVG outlines, so generated labels do not depend on a
system font installation.

## ReX math font

`rex-stix2.otf` is STIX Two Math version 2.00 b137, copied byte-for-byte from
[`rex-xits.otf` in ReTeX/ReX](https://github.com/ReTeX/ReX/blob/7bd4efc0b77c6910b79668d32824b464ee0958a3/rex-xits.otf).
Despite the upstream filename, its name table identifies STIX Two Math.

- upstream revision: `7bd4efc0b77c6910b79668d32824b464ee0958a3`
- SHA-256: `b6cf92dc8c6e75184293efd0bcfda8a641b95332c4f037866464f96b24faa614`
- license: SIL Open Font License 1.1, reproduced in `STIX-OFL.txt`

This font's private-use glyph mappings match ReX's generated STIX2 tables.
Replacing it with a newer standard STIX font without regenerating those tables
can break stretchy delimiters and other constructed glyphs. It is embedded only
when the `rex` feature is enabled and is rendered as SVG paths and rules.
