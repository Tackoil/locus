# Locus drawing-effort benchmark

Protocol **locus-drawing-effort 1.0** compares Locus with direct SVG **within the
same harness**. It measures agent effort to produce correct, complete, readable
images. [SUMMARY.md](SUMMARY.md) preserves the historical findings; generated
sources, images, captured requests and run logs are not repository inputs.

The project has one skill, `skills/locus/`, containing its instructions, UI
metadata and language reference. Benchmark tooling lives here in
`benchmarks/locus/`. Each experiment freezes the skill and benchmark separately;
only the skill files are copied into the Locus trial's drawing workspace.

## Reproduce

Use macOS or Linux, Python 3.9+ with IANA timezone data, Node 22.19+ or 24+ (Node
23 is unsupported), npm, and the repository's locked Rust toolchain. Node 26.8.1
was used for the original trials. Run these commands from the repository root:

```sh
cargo build --locked -p locus-cli --features png
python3 benchmarks/locus/benchmark.py setup
python3 benchmarks/locus/benchmark.py check
export DEEPSEEK_API_KEY='your-key'
python3 benchmarks/locus/benchmark.py run --output target/locus-skill-benchmark/reproduction-1
```

`setup` installs the exact dependency tree from `package-lock.json` into ignored
`target/locus-benchmark-tools/`; it does not install global tools. `check` verifies
harness versions, Node compatibility, Sharp and Locus PNG export without model
calls. The runner also accepts the key from the repository's ignored `.env`.
`run` makes paid DeepSeek API calls. It refuses to overwrite an experiment.

The locked tools are DSH 0.1.5-rc.2, Pi 0.84.4, OpenCode 1.18.35 and Sharp 0.35.4.
The manifest records runtime versions, platform, command/binary hashes, the
repository revision/dirty state and all benchmark input hashes. Reproduction
means the same recorded protocol and inputs, not identical model responses or
provider-cache state. Changes to prompts, checks, rubric or settings should
increment the protocol version in `protocol.json`.

Installed executables can be overridden, including a built DSH checkout:

```sh
python3 benchmarks/locus/benchmark.py check --dsh 'node /path/to/deepseek-harness/apps/cli/lib/bin.js' --pi /path/to/pi --opencode /path/to/opencode --sharp /path/to/node_modules/sharp
```

Pass the same overrides to `run` and `--resume`. Version mismatches are rejected.
Published dependency setup follows the pinned packages' native CLIs;
[DSH documentation](https://github.com/deepseek-ai/deepseek-harness),
[Pi documentation](https://github.com/earendil-works/pi/tree/main/packages/coding-agent),
and [OpenCode CLI documentation](https://opencode.ai/docs/cli/) describe them.

## Experiment protocol

| Property | Canonical setting |
| --- | --- |
| Model | `deepseek-flash` via DeepSeek Chat Completions |
| Thinking / temperature | Disabled / 0 |
| Output cap | 8,192 tokens per response |
| Harnesses | DSH headless, Pi print mode, OpenCode run mode |
| Tasks | Triangle and checkout sequence diagram |
| Workflows | Locus authoring/rendering and direct SVG XML |
| Repetitions | Three fresh sessions per task/workflow/harness: 36 trials |
| Per-trial limits | 64 forwarded provider requests; 600 seconds |
| Order | Rotate harness order; alternate workflow order for each pair across repetitions |

The triangle requires a 3–4–5 triangle, altitude, circumcircle, incircle,
right-angle marks, labels and caption. The sequence requires four participants,
11 message arrows in approved/declined alternatives, self-calls, request/return
styles, dashed lifelines, activation bars, title and legend. All six reusable
fixtures live in `tasks/`: two prompts per task and one Locus contract per task.

Locus receives the copied skill/reference, fixed IDs, at least six authored
assertions and independent validation. SVG receives a parsing gate and a preview
renderer. This compares **full workflows**, including Locus's extra context and
verification burden; it does not isolate syntax or measure human usability.
Sequence checks enforce objects/text, participant arrangement and lifelines;
arrow semantics, branch layout and activation duration require image review.

The descriptive criterion for a task/harness pair is: all Locus trials export
SVGs; Locus uses fewer total tokens and lower estimated cost; mean correctness,
completeness and overall visual score are no lower than direct SVG. The report
labels whether this criterion was met. Three trials are not statistical proof.

A smaller development run is explicitly marked `canonical: false`:

```sh
python3 benchmarks/locus/benchmark.py run --tasks triangle --harnesses pi --runs 1 --output target/locus-skill-benchmark/development-1
```

## Resume and retained evidence

```sh
python3 benchmarks/locus/benchmark.py run --resume --output target/locus-skill-benchmark/reproduction-1
```

Resume reuses completed metrics, including completed failures, without extra
model calls. It verifies settings, software identity and frozen input hashes.
An interrupted trial without final metrics is refused: preserve its raw usage
and start a fresh experiment rather than silently discarding its cost. Custom
runs must supply the same custom settings when resumed.

```text
<output>/
  manifest.json                 protocol, environment, hashes and trial order
  inputs/                       frozen skill, fixtures and runner inputs
  locus/{triangle,sequence}/    sources/SVGs, metrics, ratings.json, summary.json
  svg/{triangle,sequence}/      original SVGs, metrics, ratings.json, summary.json
  <workflow>/<task>/raw/<run>/  prompts, commands, logs, requests, usage, previews
  report.md                     final paired comparison, after image review
  workflow-summary.json         grouped metrics, after image review
```

The proxy holds the real credential; child configurations use a placeholder.
Request bodies are retained for context auditing, without authorization headers;
occurrences of the DeepSeek credential are redacted in saved bodies. Native
instructions and tool schemas remain different. The coordinator supplies the task; harnesses add native instructions/tools and
may discover applicable instruction files. The coordinator does not append the
source tree or parent chat. Later explicit file reads and tool results remain in
conversation history.

No model output is repaired by the coordinator. Checked Locus sources may be
rendered even if a harness omits export; native export status is recorded
separately. Direct SVG bytes are preserved, including malformed exports for
inspection. Failures/timeouts and their usage remain in the comparison. Exit
code 0, a file export or a contract pass alone do not certify image correctness.

## Review images and generate the report

Inspect every available SVG or `raw/<run>/preview.png`, preferably with the same
reviewer (Codex for the reference benchmark). Each task folder has a generated
`ratings.json` template. Existing images remain unscored until reviewed; missing
exports have an automatic zero score. Fill the reviewer, observations and four
scores, preserving the generated SVG hash:

```json
{
  "pi-1": {
    "svg_sha256": "preserve-the-generated-hash",
    "reviewer": "Codex, rendered image inspection",
    "scores": {"geometry": 40, "completeness": 25, "readability": 20, "styling": 15},
    "observations": "Record actual image-specific findings, not a parsing result."
  }
}
```

The example shows maximum component values, not a rating to copy blindly.
Geometry means triangle geometry or sequence-flow correctness (40);
completeness means all requested objects/text/activations (25); readability
covers label placement, clipping and clarity (20); styling covers hierarchy,
consistency and clutter (15). Ratings are subjective. Do not infer them from
successful checks. The report refuses incomplete experiments, unreviewed images,
stale hashes, missing declared SVGs or invalid scores.

```sh
python3 benchmarks/locus/benchmark.py report --output target/locus-skill-benchmark/reproduction-1
```

## Measurements

- **Tokens:** sum provider-reported input and output across all requests,
  including native background/title calls, cached input and repeated history.
  Reasoning tokens are not added twice. This is cumulative usage, not unique text.
- **Cost:** estimated CNY from cache-hit input, cache-miss input and output.
  `protocol.json` pins the 2026-10-07 pricing snapshot for comparability, **not
  current invoice rates**. Peak rates per million are ¥0.04 / ¥2 / ¥8; weekday
  09:00–12:00 and 14:00–18:00 in Asia/Shanghai, with half rates off-peak.
  Each request retains its applied rates. Missing successful-response usage or
  an unknown/lost response makes estimated cost unknown.
- **Revisions:** distinct observed content hashes after the first version.
  Locus counts checked source candidates; SVG counts versions at model-request
  boundaries and final export. Unobserved edits are missed. Bulk searches can
  produce hundreds of revisions in one tool call. Counts are lower bounds and
  **not API retries**; the two observation methods are not directly comparable.
- **Provider calls:** forwarded requests, including unsuccessful requests;
  local budget rejections are not forwarded or billed. API failures and repeated
  payload hashes are recorded separately; repeated payloads do not prove a
  transport retry.
- **Quality/exports/time:** retain every trial, report the full rubric and
  exported/native-export counts, and report measured elapsed time.

Native prompts, tool schemas, vision capability, provider cache and runtime
platform can affect results. The balanced schedule reduces order bias but
cannot reset provider caches. Small samples and one reviewer's scores do not
establish general harness rankings.

## Essential offline test

Only `test_benchmark.py` is kept as the benchmark test file. It covers billing,
request capture/redaction, immutable SVGs, independent geometry/sequence gates,
sandbox-local logging, trial ordering, version/input drift, checkpoint reuse,
ratings and the reporting criterion. It makes no DeepSeek calls.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/locus -p 'test_benchmark.py'
```

Keep protocol, dependency lock, runtime helpers, six fixtures, this test and
concise documentation in the repository. Generated experiments belong in ignored
`target/locus-skill-benchmark/`; do not copy them back into `benchmarks/locus/results/`.
