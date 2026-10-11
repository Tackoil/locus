# Locus drawing-effort benchmark

Protocol **locus-drawing-effort 1.5** compares Locus with direct SVG **within the
same harness**. It measures agent effort to produce correct, complete, readable
images. [SUMMARY.md](SUMMARY.md) records the latest full run and historical findings; generated
sources, images, captured requests and run logs are not repository inputs.

[REQUEST_ANALYSIS.md](REQUEST_ANALYSIS.md) records the latest analysis of captured
CLI requests and sources, with prioritized CLI, skill and benchmark improvements.

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
cargo install --locked --offline --debug --path crates/locus-cli --features png --root target/locus-cli-install --force
python3 benchmarks/locus/benchmark.py setup
python3 benchmarks/locus/benchmark.py check
export DEEPSEEK_API_KEY='your-key'
python3 benchmarks/locus/benchmark.py run --locus target/locus-cli-install/bin/locus --output target/locus-skill-benchmark/reproduction-1
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
| Tasks | Triangle, checkout sequence diagram and check-before-render flowchart |
| Workflows | Locus authoring/rendering and direct SVG XML |
| Repetitions | Three fresh sessions per task/workflow/harness: 54 trials |
| Per-trial limits | 64 forwarded provider requests; 600 seconds |
| Order | Rotate harness order; alternate workflow order for each pair across repetitions |

The triangle requires a 3–4–5 triangle, altitude, circumcircle, incircle,
right-angle marks, labels and caption. The sequence requires four participants,
11 message arrows in approved/declined alternatives, self-calls, request/return
styles, dashed lifelines, activation bars, title and legend. The flowchart requires
five spine nodes, a side repair node, a decision diamond, yes/no branches, a
curved repair loop, title and caption. Each task description lives once in
`tasks/{triangle,sequence,flowchart}.txt`. The two workflow templates,
`templates/locus.txt` and `templates/svg.txt`, differ only in one instruction
line. Both runners inject the same task description into `{task_prompt}` and
fill workspace/tool paths before saving the complete prompt in `raw/<run>/prompt.txt`.
Task and template hashes are recorded separately; frozen inputs include both.

Protocol 1.3 removes authored assertion requirements, fixed instrumentation IDs
and independent geometry/annotation contracts. Locus receives the copied
skill/reference and native CLI checks; SVG receives a parsing check and a
preview renderer. Image requirements and workspace rules are shared. Geometry,
diagram semantics, completeness and visual layout are scored from the images.
This compares **full workflows**, including context and native verification;
it does not isolate syntax or measure human usability. The legacy metrics field
`contract_passed` now records native Locus check or SVG parsing success.

The descriptive criterion for a task/harness pair is: all Locus trials export
SVGs; Locus uses fewer total tokens; mean correctness,
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
and start a fresh experiment rather than silently discarding its usage. Custom
runs must supply the same custom settings when resumed.

```text
<output>/
  manifest.json                 protocol, environment, hashes and trial order
  inputs/                       frozen skill, fixtures and runner inputs
  locus/{triangle,sequence,flowchart}/ sources/SVGs, metrics, ratings.json, summary.json
  svg/{triangle,sequence,flowchart}/   original SVGs, metrics, ratings.json, summary.json
  <workflow>/<task>/raw/<run>/  prompts, commands, logs, requests, usage, previews
    work/.locus/rejected-requests/ debug CLI failures with sources and diagnostics
    work/.locus/requests/          every debug CLI request, including full source copies
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
code 0, a file export or a native check pass alone do not certify image correctness.
Use the debug CLI built by the reproduction command to collect rejected Locus
requests. The runner sets `LOCUS_REJECTED_REQUESTS_DIR` inside each trial's work
directory so native sandboxes allow the writes and rejected source evidence is
retained.
These archives are separate from the proxy's captured provider request bodies.
Protocol 1.5 enables `LOCUS_REQUESTS_DIR` for all native and coordinator CLI calls,
retaining successful requests, usage errors, help/version calls, full original
source bytes, stdin and edited candidates. Failed requests also remain in the
existing rejection archive. Both workflow prompts prohibit `cargo run` and use
installed executables. Request capture requires a debug build; release builds
do not include this instrumentation.

### Filesystem searches

Protocol 1.3 removes the benchmark search-command wrappers and Pi tool-call
extension introduced in protocol 1.2. Searches use native harness permissions;
there is no benchmark-added filesystem search enforcement. Pi extensions remain
disabled. Shared workspace instructions still apply to both workflows.

Earlier experiments retain their usage and cannot be resumed under these
changed inputs. Start a fresh output folder for protocol 1.5.

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
Geometry means triangle geometry or sequence/flowchart correctness (40);
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
  Missing successful-response usage or an unknown/lost response marks token
  accounting incomplete; totals include only reported usage.
- **Revisions:** distinct observed content hashes after the first version.
  Locus counts checked source candidates; SVG counts versions at model-request
  boundaries and final export. Unobserved edits are missed. Bulk searches can
  produce hundreds of revisions in one tool call. Counts are lower bounds and
  **not API retries**; the two observation methods are not directly comparable.
- **Provider calls:** forwarded requests, including unsuccessful requests;
  local budget rejections are not forwarded. API failures and repeated
  payload hashes are recorded separately; repeated payloads do not prove a
  transport retry.
- **Quality/exports/time:** retain every trial, report the full rubric and
  exported/native-export counts, and report measured elapsed time.

Native prompts, tool schemas, vision capability, provider cache and runtime
platform can affect results. The balanced schedule reduces order bias but
cannot reset provider caches. Small samples and one reviewer's scores do not
establish general harness rankings.

## Essential offline test

Only `test_benchmark.py` is kept as the benchmark test file. It covers token accounting,
request capture/redaction, immutable SVGs, native source checks, matched prompts,
sandbox-local logging, trial ordering, version/input drift, checkpoint reuse,
ratings and the reporting criterion. It makes no DeepSeek calls.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/locus -p 'test_benchmark.py'
```

Keep protocol, dependency lock, runtime helpers, three task descriptions, two workflow templates, this test and
concise documentation in the repository. Generated experiments belong in ignored
`target/locus-skill-benchmark/`; do not copy them back into `benchmarks/locus/results/`.
