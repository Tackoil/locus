# Locus beta 6 drawing benchmarks

See [SUMMARY.md](SUMMARY.md) for the completed historical-harness run and its
measurement limits, followed by the separate Codex development results.
See [FAILURE_ANALYSIS.md](FAILURE_ANALYSIS.md) for case-level causes, a reduced
solver reproduction, and the workflow-contamination correction. The completed
run's assigned SVG arm includes a Locus-rendered image and is exploratory.

## Retired provider benchmark

The provider-backed drawing HTTP service, credential proxy, native harness
launchers, and their integration tests have been removed. No benchmark command
starts an HTTP server or makes paid model calls.

[SUMMARY.md](SUMMARY.md) and [FAILURE_ANALYSIS.md](FAILURE_ANALYSIS.md) retain the
historical observations and their limitations. `audit.py`, `report.py`, and
`workflow_report.py` remain offline readers for saved evidence;
`provider_protocol.json` retains the historical scoring rubric. References to
provider protocols in those records describe the original experiments.

Use the standalone workflow below for new drawing trials.

## Separate Codex subagent development harness

Protocol **locus-drawing-effort 2.0** compares the restored Locus 0.0.0-beta.6
skill with native handwritten SVG in independent agent sessions. The three task
descriptions are preserved verbatim from the historical protocol 1.5 before
commit `bf323b7`: a 3–4–5 geometry construction, checkout sequence diagram and
check-before-render flowchart. The common task and workspace rules are shared;
only authoring instructions and Locus documentation differ between workflows.

The new runner uses Python's standard library. It does not install dependencies,
make paid provider requests, or fabricate token, request, revision or cost data.
Independent Codex subagents are dispatched by the coordinator and their
untouched artifacts are recorded with `benchmark.py`. Agent/model identity and
fresh-session execution remain coordinator responsibilities. This compares full
workflows, not syntax alone. Its protocol, metrics, and reports remain separate
from the retired provider-metered historical harness.

## Prepare and run

From the repository root, build the beta 6 CLI with `cargo build --release --locked
-p locus-cli`. A compatible Node executable and `sharp` module are required for
PNG previews. Install the locked preview dependency with
`npm ci --prefix benchmarks/locus`. The default Sharp path is
`benchmarks/locus/node_modules/sharp`; pass `--sharp /absolute/path/to/sharp`
and `--node /absolute/path/to/node` for another installation. The runner does
not download dependencies.

```sh
python3 benchmarks/locus/benchmark.py prepare --output target/locus-skill-benchmark/beta6-run-1
```

Preparation refuses an existing output directory, freezes task prompts, skill,
protocol, runner and rasterizer, and hashes all frozen inputs and the Locus
binary, Node executable and Sharp package metadata. It records the repository
revision and dirty status. Each trial has
`trials/<agent>-<task>-<workflow>-<repetition>/prompt.txt` and a disposable `work/`
directory. Only Locus trials receive the frozen skill. Use `--repetitions 3` for
three independent trials per task/backend (18 total); the default is a six-trial
development run. Backend ordering alternates across tasks and repetitions.

Dispatch each saved prompt to a fresh agent session without parent conversation
history. Agents must read only their own workspace and supplied frozen tooling,
inspect a raster preview, and produce `figure.svg`; Locus also produces
`figure.locus`. Retain failure diagnostics and the agent transcript. Record
coordinator-measured wall time if available, including failed/time-out trials:

```sh
python3 benchmarks/locus/benchmark.py record --output target/locus-skill-benchmark/beta6-run-1 --trial codex-triangle-locus-1 --elapsed-seconds 120
```

Use `--status failed` or `--status timeout` when appropriate. Omit elapsed time
when it was not measured. Never copy or repair another trial's output. A pilot
using a different prompt belongs in a separate, explicitly labeled experiment.

`record` refuses to overwrite metrics or accept changed frozen inputs, prompts
or CLI binaries. Trial copies of the skill must match the frozen input. Locus
trials run the native check and a separate verification
render; its hash must match the agent's original SVG. SVG trials check XML and
the SVG namespace. Neither check measures image correctness. All provider usage
fields are `null`; source file size is a descriptive artifact metric, not an
effort metric.

## Blind image review and report

After **all trials finish**, create the review packet:

```sh
python3 benchmarks/locus/benchmark.py review --output target/locus-skill-benchmark/beta6-run-1
```

Give a reviewer only `review/`, containing opaque image IDs, SVGs, freshly rendered
PNG previews, render diagnostics, task descriptions and `ratings.json`. The
review packet re-rasterizes the recorded SVG bytes rather than trusting a
potentially stale agent preview. Keep `review-key.json`, original
sources, agent reports and timings private until scoring finishes. Style may
still reveal the workflow, so this is source blinding rather than guaranteed
workflow blinding. Fill actual image-specific observations, reviewer identity,
and all rubric components: geometry/diagram correctness (40), completeness
(25), readability (20), styling (15). Preserve the supplied SVG hashes.

```sh
python3 benchmarks/locus/benchmark.py report --output target/locus-skill-benchmark/beta6-run-1 --require-reviewed
```

Every scheduled trial must have recorded metrics. Missing exports receive zero;
available exports require review. Reports reject stale image hashes, changed
inputs, changed Locus sources, changed copied skills, missing recorded images
and invalid scores. Without `--require-reviewed`,
the report labels unreviewed images explicitly. Timings include orchestration
effects and a small sample cannot establish general rankings. Provider token
efficiency and cost remain unmeasured.

Generated runs belong under ignored `target/locus-skill-benchmark/`. Commit only
the protocol, tasks, runner, tests, concise documentation and an
honest summary of actual observations.

## Offline verification

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/locus -p 'test_benchmark.py'
```
