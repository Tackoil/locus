# Bundled fonts

`NotoSans-Regular.ttf` is the deterministic default face used by
`PlaintextEngine`. It was downloaded from the official
[`notofonts/noto-fonts`](https://github.com/notofonts/noto-fonts) repository:

- upstream path: `hinted/ttf/NotoSans/NotoSans-Regular.ttf`
- SHA-256: `b85c38ecea8a7cfb39c24e395a4007474fa5a4fc864f6ee33309eb4948d232d5`
- license: SIL Open Font License 1.1, reproduced in `OFL.txt`

The font is rendered as SVG outlines, so generated labels do not depend on a
system font installation.

## CJK fallback font

`NotoSansCJKsc-Regular.otf` is Noto Sans CJK SC Regular version 2.004,
downloaded byte-for-byte from the official
[`notofonts/noto-cjk`](https://github.com/notofonts/noto-cjk) repository.

- upstream release: `Sans2.004`
- upstream revision: `523d033d6cb47f4a80c58a35753646f5c3608a78`
- upstream path: [`Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf`](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf)
- size: 16,437,364 bytes
- SHA-256: `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`
- copyright: 2014–2021 Adobe, as recorded in the font's name table
- license: SIL Open Font License 1.1, reproduced in `CJK-OFL.txt` from
  [upstream `LICENSE`](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/LICENSE)

This is the full language-specific OTF with 65,535 glyphs, not the smaller
region-specific `NotoSansSC` subset. It includes Chinese ideographs, Japanese
kana, and Korean Hangul. Shared Han characters use Simplified Chinese glyph
forms by default; selecting other regional forms requires language tagging and
the OpenType `locl` feature. Full upstream coverage does not mean every Unicode
CJK extension character is supported. See the
[upstream deployment guide](https://github.com/notofonts/noto-cjk/blob/523d033d6cb47f4a80c58a35753646f5c3608a78/Sans/README.md#language-specific-otfs)
for the distinction between language-specific fonts and regional subsets.

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
