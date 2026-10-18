Roboto Serif Regular TrueType font

Copyright 2020 The Roboto Serif Project Authors
https://github.com/googlefonts/RobotoSerif

Official variable font source:
https://raw.githubusercontent.com/google/fonts/main/ofl/robotoserif/RobotoSerif%5BGRAD,opsz,wdth,wght%5D.ttf
Source SHA-256: 351ced75f3851806aa6d846b669361521eb1925cfc530396df9c1a1b77061ddb

RobotoSerif-Regular.ttf is a static instance of that font, generated with
HarfBuzz at weight 400, width 100, optical size 20, and grade 0:
hb-subset RobotoSerif.ttf --keep-everything --variations='wdth=100,opsz=20,wght=400,GRAD=0' --output-file=RobotoSerif-Regular.ttf

Instance SHA-256: bd077efb9e271d47b39b52034257383d6eaf2b6042a7a5596cd712139642516c
License: SIL Open Font License 1.1; see RobotoSerif-OFL.txt.

The logo example loads the font directly through Locus's PlaintextEngine.
Both SVG and PNG are rendered by Locus from assets/locus-banner.locus.

Regenerate from the repository root:
cargo run -p locus-render --example logo > assets/locus-banner.svg
cargo run -p locus-render --features png --example logo -- --png > assets/locus-banner.png
