# Your first Locus figure

Locus is a relational language and CLI toolchain for deterministic plane geometry and technical diagrams. Declare what should be true, let the solver place the geometry, then check your assertions before rendering.

This project is under active development. The current frontend accepts **`locus 0 beta 6`** files. Start with the checked examples and consult the [language specification](../../spec/specification.md) for the exact contract.

## Build from source

Install Rust with Cargo and a toolchain that supports the repository’s `rust-toolchain.toml`. Clone the repository, then build the CLI:

```sh
git clone https://github.com/Tackoil/locus.git
cd locus
cargo build --release -p locus-cli --locked
```

The executable is `target/release/locus`. To install it on your Cargo path instead:

```sh
cargo install --path crates/locus-cli --locked
```

## Declare relationships

Save this as `triangle.locus`. The horizontal and vertical distances define a 3–4–5 triangle. No absolute coordinates are needed.

```locus
locus 0 beta 6
style textbook

point A()
point B() where { align_center_y(A, B), hdist(A, B) == 4 }
point C() where { align_center_x(A, C), vdist(A, C) == 3 }

polygon tri(A, B, C)
point M = midpoint(B, C)
circle circum(M, dist(M, A)) [construction]
right_angle corner(A, B, C)

label lA(A, "A") { dir: SW }
label lB(B, "B") { dir: SE }
label lC(C, "C") { dir: NW }
label lM(M, "M") { dir: NE }

assert dist(B, C) == 5
assert area(tri) == 6
assert on(A, circum) and on(B, circum) and on(C, circum)
```

## Check, then render

Validate the source and execute its assertions. Render after the check succeeds:

```sh
target/release/locus check triangle.locus
target/release/locus render triangle.locus -o triangle.svg
```

Open `triangle.svg` in a browser. The output uses bundled fonts and does not need an external TeX installation.

## Inspect what you made

Request focused measurements or machine-readable feedback:

```sh
target/release/locus inspect triangle.locus --distance B C --json --compact
target/release/locus check triangle.locus --layout --strict --json --compact
```

The strict layout check can fail on warnings, including incomplete preference settling. Read the reported diagnostics and inspect the image before treating a diagram as finished.

## Explore the examples

Open [the triangle demonstration](../../examples/demonstrations/point_relations.locus), try [the agent workflow](../../examples/demonstrations/flowchart_agent_loop.locus), or read the [demonstration guide](../../examples/demonstrations/README.md) for all 43 figures.

## Choose an export

SVG is the default. TikZ is available directly; PNG requires the optional feature:

```sh
target/release/locus render triangle.locus --tikz -o triangle.tex
cargo run --release -p locus-cli --features png -- render triangle.locus --png -o triangle.png
```

See the [project and CLI guide](../../README.md) for rendering options, diagnostics, timing reports, and offline render-trace viewers.

## Keep going

- [Authoring reference](../../skills/locus/references/language.md): a practical syntax and geometry guide.
- [Language specification](../../spec/specification.md): the complete normative contract.
- [Working with agents](../../skills/locus/SKILL.md): the project’s checked authoring workflow.
- [Zed integration](../../ecosystem/zed/README.md): syntax highlighting and the language server.

Locus’s own code is dual-licensed under MIT or Apache-2.0. Bundled dependencies and fonts retain their own terms; the repository’s third-party notices cover them.
