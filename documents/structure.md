# Locus — Rust Project Structure

Rust is a great fit here: deterministic floating-point behavior, excellent CLI ecosystem, single static binary (important for agents — `cargo install locus` or a downloaded binary, no runtime deps), and strong parser tooling.

Here's the structure I'd commit to.

---

## 1. Workspace layout

A Cargo workspace with focused crates along the natural seams — syntax, semantics, layout, rendering, CLI. The seams matter because they enforce the architecture rules from the spec (e.g. geometry must not depend on layout):

```
locus/
├── Cargo.toml                     # [workspace] members, shared deps, lints
├── rust-toolchain.toml            # pin toolchain for reproducible builds
├── documents/
│   ├── plan.md                    # historical implementation plan
│   └── structure.md               # this document
├── spec/
│   ├── specification.md           # the normative 0 beta 5 relational spec
│   └── test/                      # *.locus demonstrations and stress figures
├── crates/
│   ├── locus-syntax/              # lexer, parser, AST, formatter, spans
│   ├── locus-core/                # geometry kernel, constraint solver, evaluator, inspect/assert
│   ├── locus-layout/              # label placement, TeX box measurement
│   ├── locus-render/              # SVG / TikZ backends
│   ├── locus-tex/                 # TeX fragment → measured SVG box
│   ├── locus-lsp/                 # editor language server over stdio
│   └── locus-cli/                 # the `locus` binary
├── fixtures/
│   ├── figures/                   # *.locus test figures (incl. the stress tests)
│   ├── golden/                    # expected SVG / inspect-JSON outputs
│   └── errors/                    # invalid files + expected diagnostics
└── tools/
    └── locus-tmgrammar/           # TextMate grammar for editor highlighting (later)
```

Dependency direction (enforced by crate boundaries — a crate physically cannot import downstream):

```
locus-syntax  ──►  locus-core  ──►  locus-layout  ──►  locus-render
      │                │                 ▲
      │                │                 │
      └────────────────┴──── locus-tex ──┘
                             (measurement only)
locus-cli  ──►  all of the above
locus-lsp  ──►  locus-syntax + locus-core
```

Note **`locus-core` does not depend on `locus-layout` or `locus-render`** — this is the spec's "geometry never depends on layout" rule turned into a compile-time guarantee. The language server also stays independent of layout and rendering: editor feedback must remain text-first and must not start a rendering toolchain in the background.

---

## 2. Crate-by-crate breakdown

### 2.1 `locus-syntax` — the language frontend

```
locus-syntax/src/
├── lib.rs
├── span.rs          # Span { line, col, byte range }; everything carries one
├── lexer.rs         # hand-written lexer (tokens: ident, number+unit, string, TeX, punct)
├── ast.rs           # File, Statement, OpStmt, AssertStmt, Expr, Constructor, Props
├── parser.rs        # recursive descent, per-line recovery
├── format.rs        # canonical formatter (locus fmt), incl. `=` alignment
├── diag.rs          # Diagnostic { code, span, message, notes, suggestion }
└── json.rs          # AST → interchange JSON (fmt --json), serde
```

Key decisions:

- **Hand-written recursive descent**, not a parser generator. The grammar is small, line-oriented recovery is the whole point (one bad line must not kill the file), and hand-written parsers produce the best diagnostics. `nom`/`pest` fight you on both.
- **Spans on every AST node.** All downstream errors (even construction errors in `locus-core`) must point back to source lines — so spans flow through the entire pipeline.
- **`did you mean` suggestions** via a small edit-distance check against known constructor/prop names (the `incircel` → `incircle` case). Cheap, and disproportionately useful for agents.
- AST types are `serde::Serialize` (for `fmt --json`) but the parser never uses serde — the DSL is parsed by hand.

```rust
pub struct OpStmt {
    pub kind: Kind,             // Point, Line, Segment, Circle, ...
    pub id: Ident,
    pub ctor: Constructor,      // name + Vec<Expr>
    pub attr: Option<StyleAttr>,
    pub props: Props,
    pub span: Span,
}

pub enum Expr {
    Number { value: f64, unit: Option<AngleUnit>, span: Span },
    Tex { value: String, span: Span },
    Ref(Ident),
    Call { name: Ident, args: Vec<Expr>, span: Span },
    Property { base: Box<Expr>, name: Ident, args: Vec<Expr>, span: Span },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    // ...
}
```

### 2.2 `locus-core` — geometry kernel + evaluator (the heart)

```
locus-core/src/
├── lib.rs
├── geom/
│   ├── mod.rs       # Point, Vec2, Line, Ray, Segment, Circle, Arc, Polygon
│   ├── intersect.rs # all type-pair intersections + branch ordering rules
│   ├── construct.rs # foot, project, tangent_line, incircle, circumcircle, ...
│   └── measure.rs   # dist, angle, dir_angle, area, perimeter
├── eval/
│   ├── mod.rs       # top-to-bottom evaluation, dependency graph
│   ├── env.rs       # id → ResolvedObject environment
│   ├── expr.rs      # expression evaluator (Value: Scalar | Angle | ObjectRef | Bool)
│   ├── deps.rs      # dependency extraction, orphan detection for remove/update
│   └── error.rs     # E2xx construction errors with measured values + spans
├── scene.rs         # Scene = parsed File + ResolvedScene; recompute API
├── inspect.rs       # measurement tables, incidence detection, diagnostics
├── assert.rs        # predicate eval, subterm actual-value reporting
└── tol.rs           # relative tolerance (1e-9 × bbox diameter), one place only
```

Key decisions:

- **`f64` everywhere, no exact arithmetic.** Rationals/symbolic angles were considered in the earlier draft; for the spec's tolerance model (relative epsilon) doubles are sufficient and vastly simpler. The `tol.rs` module is the *single* place tolerance logic lives — never inline epsilons.
- **The relational solver (spec §7) lives here**, in a `solve/` module: Levenberg–Marquardt with the fixed schedule from the spec, Jacobian-rank DOF accounting, and the under/over-constraint reporters. Math functions come from the pinned `libm` crate — never system libm — so solves are bit-identical across platforms (spec §7.6).
- **Determinism discipline (normative per spec §7.6):** use `IndexMap` / `BTreeMap` only — **never `HashMap` iteration** anywhere that affects output. Unknown and equation ordering is file order; branch ordering rules for explicit `k` live in `intersect.rs` next to the intersection math, with unit tests per type-pair.
- **`Value` enum for expressions**, with `Angle` as a distinct variant from `Scalar` — this is how the "mandatory `deg`/`rad`" rule is enforced at evaluation, not just parse time (e.g. `rotate(P, O, 2 * 30deg)` type-checks; `rotate(P, O, 60)` fails).
- `inspect.rs` implements the spec's incidence detection (tangency, on-ness, ratio detection along shared lines) — this is the agent's eyes, treat it as a first-class module with its own fixture tests, not an afterthought.

```rust
pub enum ResolvedObject {
    Point(Point),
    Line(Line), Ray(Ray), Segment(Segment),
    Circle(Circle), Arc(Arc),
    Polygon(Polygon),
    Annotation(AnnotationSpec),   // unresolved position — layout's job
}

pub struct ResolvedScene {
    pub objects: IndexMap<Ident, ResolvedObject>,  // insertion order = file order
    pub bbox: BBox,                                 // geometry-only bbox
}
```

### 2.3 `locus-layout` — annotation placement

```
locus-layout/src/
├── lib.rs
├── candidates.rs    # 8 dirs × 2 radii + domain preferences (external bisector etc.)
├── score.rs         # stroke/label/margin overlap penalties
├── assign.rs        # greedy most-constrained-first, deterministic tie-break
├── boxes.rs         # label box geometry (from locus-tex measurements or plain text)
└── report.rs        # residual collisions → diagnostics for inspect
```

Small on purpose. Takes `ResolvedScene` + measured text boxes, returns `LayoutScene` (every annotation gets a final box + leader info). No solver dependency — label placement stays the candidate/scoring heuristic per spec §8.1. The geometric constraint solver is `locus-core`'s (spec §7); layout runs strictly after it and must never feed back into geometry.

### 2.4 `locus-tex` — TeX measurement & rendering

This is the one genuinely awkward dependency in a pure-Rust stack, so isolate it behind a trait:

```rust
pub trait TexEngine {
    /// Render a TeX fragment; returns SVG body + measured box (em-relative).
    fn render(&self, tex: &str, size: FontSize) -> Result<TexBox, TexError>;
}
```

Implementation options, in order of preference:

1. **`ReX`** (Rust TeX math renderer) — pure Rust, renders to SVG with real font metrics. Covers the spec's KaTeX-subset ambition; its supported-command list *becomes* your spec Appendix whitelist.
2. Fallback feature flag: shell out to a bundled KaTeX via Node/QuickJS — only if ReX's coverage proves insufficient.
3. A `plaintext` engine for `label(A, "A")` non-TeX labels — just font metrics via `rustybuzz`/`ttf-parser`.

The trait means the rest of the codebase never knows which engine ran, and golden tests can use a deterministic mock.

### 2.5 `locus-render` — backends

```
locus-render/src/
├── lib.rs
├── viewport.rs      # auto-fit (geometry ∪ label boxes), margin, y-flip
├── style.rs         # `textbook` preset; semantic class → stroke/dash/color
├── svg.rs           # primary backend; deterministic float formatting!
├── tikz.rs          # secondary backend
└── png.rs           # feature-gated: resvg rasterization of our own SVG
```

- **Deterministic float formatting** is the subtle requirement for byte-identical SVG: fixed precision, `ryu`-based formatting, no locale involvement. Write this once in `svg.rs` and test it.
- PNG via `resvg` (feature `png`) keeps the default build lean; TikZ backend emits into a `tikzpicture` using the same resolved layout, so TikZ output matches the SVG geometrically.

### 2.6 `locus-cli` — the binary

```
locus-cli/src/
├── main.rs          # clap derive; subcommand dispatch
├── commands/
│   ├── init.rs  add.rs  update.rs  remove.rs
│   ├── describe.rs  inspect.rs  check.rs
│   ├── fmt.rs   render.rs
├── output.rs        # human (miette-style, source-annotated) vs --json
└── exit.rs          # ExitCode mapping: 0/1/2/3 per spec §9.2
```

- **`clap`** (derive) for args; **`miette`** for the human diagnostic rendering (source snippet + caret + suggestion — exactly the spec §7.5 format, nearly free).
- `output.rs` owns the human/JSON dual-format rule: every command's result is a `Serialize` struct first, then either serialized (`--json`, to stdout) or pretty-printed. Never format prose ad-hoc inside commands — this keeps agent output stable.
- Each subcommand is a thin function: parse → eval → (layout → render) → report. All logic lives in library crates, so the CLI stays testable end-to-end via `assert_cmd`.

### 2.7 `locus-lsp` — editor language server

```
locus-lsp/src/
├── main.rs          # stdio entry point and LSP initialization handshake
├── server.rs        # request/notification dispatch; shutdown lifecycle
├── state.rs         # open-document snapshots and workspace configuration
├── document.rs      # versioned text, line index, UTF-16 ↔ byte-span conversion
├── capabilities.rs  # one authoritative advertised-capabilities value
├── diagnostics.rs   # stable Locus diagnostics → LSP diagnostics
└── handlers/
    ├── completion.rs  hover.rs  definition.rs  references.rs  rename.rs
    ├── formatting.rs  symbols.rs  semantic_tokens.rs
    └── code_actions.rs
```

Use the synchronous `lsp-server` transport scaffold and `lsp-types` protocol models. Keeping the transport loop small avoids committing the analysis crates to an async runtime; expensive evaluation can move to a bounded worker queue later without changing the protocol layer.

Initial capabilities:

- **Live diagnostics** on open/change/save. Syntax and static diagnostics come from `locus-syntax`; construction diagnostics are added from `locus-core` only when the document has a valid evaluable prefix. LSP diagnostic `code` uses the same stable `E1xx`–`E4xx` registry, and structured details live in `Diagnostic.data` using the normative diagnostic schema.
- **Context-aware completion** for statement kinds, constructors, previously defined ids, properties, enum values, predicates, and units. Completion tables are exported by the language crates; the server must not maintain a second language vocabulary.
- **Hover and navigation** for definitions, inferred object kinds, constructor signatures, resolved measurements when evaluation succeeds, go-to-definition, references, and file-local rename.
- **Canonical formatting** through `locus-syntax`; document and range formatting share the CLI formatter and never implement editor-specific formatting rules.
- **Document symbols and semantic tokens** for operations, assertions, references, numbers/units, strings, TeX literals, comments, properties, and semantic style attributes. The later TextMate grammar is only a fallback for clients without semantic-token support.
- **Safe code actions** sourced directly from diagnostics, initially spelling replacements, missing mandatory angle units when unambiguous, and canonical property/attribute replacements. No action silently changes geometry or chooses a construction branch.

The first implementation reparses the full in-memory document after each debounced edit. Files are expected to be small, and the line-oriented parser already provides local recovery. Every analysis result carries the LSP document version; stale results are discarded before publication. `document.rs` owns all UTF-16 position conversion so internal spans remain byte-based and identical to CLI spans.

The server supports standard LSP 3.17 over stdio and introduces no custom requests in the first release. Editor extensions should launch the `locus-lsp` binary and consume standard capabilities instead of calling internal crates. This keeps VS Code, Zed, Neovim, and other clients thin.

---

## 3. Workspace `Cargo.toml` essentials

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indexmap = { version = "2", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
thiserror = "2"
miette = { version = "7", features = ["fancy"] }
ryu = "1"
lsp-server = "0.10"
lsp-types = "0.97"
insta = "1"            # dev: snapshot testing
assert_cmd = "2"       # dev: CLI end-to-end
proptest = "1"         # dev: property tests

[workspace.lints.clippy]
# deny the determinism foot-guns
disallowed-types = "deny"   # configure: std::collections::HashMap in output paths
```

Error strategy: `thiserror` enums per crate (`SyntaxError`, `ConstructError`, `LayoutError`), each carrying a `Span` and stable error code (`E1xx` syntax, `E2xx` construction, `E3xx` dependency, `E4xx` layout) — the codes go in the spec appendix and never get renumbered.

---

## 4. Testing strategy (this project lives or dies by it)

| Layer | Method |
|---|---|
| Lexer/parser | unit tests + `proptest` round-trip: `parse(fmt(parse(x))) == parse(x)` |
| Formatter | idempotence: `fmt(fmt(x)) == fmt(x)` |
| Geometry kernel | unit tests per constructor with known-answer figures; branch-ordering tests per intersect type-pair |
| Solver | known-answer relational fixtures (tangency, alignment, mixed); DOF-accounting tests (under/over/conflicting); hint-branch-selection tests (same relations, different hints → different documented branches) |
| Determinism | run full pipeline twice on every fixture, assert byte-equal SVG; CI runs on Linux+macOS to catch platform float drift early |
| Golden files | `spec/test/demonstrations/*.locus` → `insta` snapshots of `describe --json`, `inspect --json`, and rendered SVG |
| Error quality | `fixtures/errors/*.locus` → snapshot the *diagnostics* (error UX is a feature; test it like one) |
| CLI | `assert_cmd`: exit codes 0/1/2/3, stdin `add` flow, the full agent loop from spec §9.5 |
| Language server | in-memory JSON-RPC transcript tests for initialization, edits, diagnostics, completion, navigation, formatting, rename, shutdown, stale-version suppression, and UTF-16/byte-span edge cases |

The compact tangent-circles and angle-bisector figures live in `spec/test/demonstrations/` as the first golden tests; their expanded counterparts live in `spec/test/stress/`.

---

## 5. Suggested build order

1. **`locus-syntax`** — lexer → parser → formatter → JSON export. Testable standalone; `locus fmt` works with zero geometry.
2. **`locus-core` minimal** — `free`, `midpoint`, `seg`, `circle`, `intersect` + evaluator + `describe`. First vertical slice.
3. **`locus-cli` skeleton** — `init/add/describe/fmt/check` (asserts on the minimal predicates). *The agent loop is now testable end-to-end.*
4. **`locus-lsp` baseline** — document synchronization, shared diagnostics, completion, hover, go-to-definition, formatting, symbols, and transcript tests. Build this after the first vertical slice so editor behavior exercises the real parser/evaluator rather than mocks.
5. **Fill the kernel and LSP semantics together** — remaining constructors, `inspect` incidences, predicates, references/rename, semantic tokens, and safe code actions. Each new language feature ships with CLI and LSP coverage in the same change.
6. **`locus-tex` + `locus-layout` + `locus-render`** — labels last, since everything upstream is verifiable textually without them. The language server remains rendering-independent.

Steps 3 and 4 coming early are deliberate: you get both a dogfoodable agent workflow and editor feedback before touching any rendering, which matches the project's text-first philosophy.

---
