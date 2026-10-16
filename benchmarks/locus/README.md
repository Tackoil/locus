# Locus beta 6 drawing benchmarks

See [SUMMARY.md](SUMMARY.md) for the completed historical-harness run and its
measurement limits, followed by the separate Codex development results.
See [FAILURE_ANALYSIS.md](FAILURE_ANALYSIS.md) for case-level causes, a reduced
solver reproduction, and the workflow-contamination correction. The completed
run's assigned SVG arm includes a Locus-rendered image and is exploratory.

## Restricted DSH, Pi, and OpenCode harnesses: provider protocol 2.0

`provider_benchmark.py` now uses a source-only HTTP boundary. Provider protocol
**locus-provider-http 2.0** replaces exploratory protocol 1.6; it cannot resume or be combined with
that evidence as a clean comparison. It uses pinned DSH 0.1.5-rc.2, Pi 0.84.4,
OpenCode 1.18.35, and Sharp 0.35.4
dependencies. The model is `deepseek-flash`, thinking disabled, temperature 0,
with an 8,192-token response cap. The default matrix is three tasks, two drawing
workflows, three harnesses, and three fresh repetitions: **54 trials**. Each
trial has a 64-request budget and a 600-second agent timeout.

No new paid benchmark has been run for this change. The 54 historical trials,
original metrics, ratings and frozen inputs are preserved. Their contamination
and timing limits still apply; new code does not retroactively repair them.

For a separately authorized future run, use the repository's ignored `.env` or the `DEEPSEEK_API_KEY` environment
variable for the existing provider credential. `run` makes paid provider calls;
`setup` and `check` do not. The localhost proxy holds the credential; child
agents receive only its placeholder credential. Request snapshots redact the
real key and retain provider-reported usage, failures, and repeated payloads.

```sh
cargo build --locked -p locus-cli --features png
cargo install --locked --offline --debug --path crates/locus-cli --features png --root target/locus-cli-install --force
python3 benchmarks/locus/provider_benchmark.py setup
python3 benchmarks/locus/provider_benchmark.py check --locus target/locus-cli-install/bin/locus
# Paid run only after fresh authorization:
python3 benchmarks/locus/provider_benchmark.py run --locus target/locus-cli-install/bin/locus --output target/locus-skill-benchmark/provider-http-v2-fresh
```

Skip `setup` when the pinned local tools already exist. A debug CLI is needed
for complete native request capture, including successful invocations; release
builds omit that instrumentation. The runner requires localhost sockets and,
for a paid run, network access to the provider.

Agents receive one tool: `drawing` (DSH/Pi) or `scoped_drawing` (OpenCode).
It submits complete source strings to a trial-private local service for
check/describe/inspect/render/submit. The coordinator supplies the skill and
language reference as message content only to Locus trials. SVG trials get
only direct SVG operations and task text; no Locus skill, generator or other
workflow's input tree. Use `submit` to select the final complete source. The
coordinator performs a strict layout check and exports that source, without
repairing it. See [the HTTP API/lifecycle](../../scripts/LOCUS_HTTP.md).

Actual native restrictions, verified against the installed pinned tools:

| Harness | Enforced boundary |
| --- | --- |
| Pi | `--tools drawing`, disabled extension/context/skill/template discovery, exactly one explicit extension; tool-call guard rejects other names. |
| DSH | Cordis home patch disables every `tool-*` producer in pinned base/headless rows, code runtime, skill filesystem, context instructions, plan-mode exit tool and auxiliary title/spill plugins; loads only a scoped drawing plugin with an execution guard. Unknown launcher/composition is refused. |
| OpenCode | `--pure`, permissions `* = deny` with only `scoped_drawing = allow`, tool map `* = false` with only that MCP tool enabled, and one stdio bridge to the fixed HTTP capability; auxiliary title/summary agents disabled. |

The proxy checks the exact advertised tool catalog before forwarding a provider
request; any extra/missing tool gets 403 and `tool-policy-failure.json` rather
than a paid request. There is no shell/curl bridge. Native runtime access to its
configuration/logs remains available to trusted harness code. These are enforced
model tool restrictions, **not OS-level denial** of the harness process. A new
harness version requires fresh offline verification; unsupported controls fail
closed rather than falling back to filesystem tools. OpenCode permission and
MCP behavior is also described in its [official tools](https://opencode.ai/docs/tools/)
and [MCP documentation](https://opencode.ai/docs/mcp-servers/).

`author_seconds`/`elapsed_seconds` cover only the native author process;
`tool_seconds` measures service operations within that interval (and is a
component of it), `observer_seconds`/`observer_solves` are zero,
`shutdown_seconds` records service teardown/quiescence, and
`final_export_seconds` covers trusted export. Provider request duration remains
separate. There are no synchronous observer solves. Render does not prepend a
check; PNG reuses SVG, and final export reuses exact cached checks/renders.
Uncached semantically different operations can still solve independently.
Every check records source bytes/hash, requested options and native diagnostics.
The legacy PATH wrapper is unused by protocol 2.0 but preserves exact original
commands/options and captures one execution for historical regression coverage.

The pinned benchmark model configuration is text-only. Native tool feedback
contains artifact MIME/hash metadata, not base64 or image content. The service
and coordinator retain the actual SVG/PNG bytes for export and human review.
Agents can inspect structured geometry/diagnostics; these smokes do not claim
visual PNG inspection by this provider. Human image ratings and task-semantic
review remain necessary.

Use `--workers 3` to run up to three independent trials concurrently. Worker
count is recorded; concurrent runs differ from the historical sequential
schedule and are labeled noncanonical. For a smaller development run, use
`--tasks triangle --runs 1`; it still runs both workflows on all three harnesses.
Tool discovery can be overridden with `--dsh`, `--pi`, `--opencode`, `--node`,
and `--sharp`.

Inputs (including bridges/service source), settings, software identities, and trial order are frozen in the run
manifest. Completed trials, including failures, may be reused with `--resume`
and exactly the same settings. An interrupted trial without final metrics is
refused rather than silently discarding provider usage. Never repair model
drawings in the coordinator. Source/check logs, native request archives, raw
provider requests, usage, SVGs, and previews stay in the generated run directory.

Keep review notes outside the experiment until **every trial has finished**.
Inspect available images against their task descriptions, fill each task's
`ratings.json` with actual observations and the 40/25/20/15 rubric, and preserve
the supplied SVG hashes. Missing exports receive zero. Then generate the paired
comparison:

```sh
python3 benchmarks/locus/provider_benchmark.py report --output target/locus-skill-benchmark/beta6-provider-run-1
```

The descriptive historical criterion requires fewer total Locus tokens, all
Locus exports, and mean correctness, completeness, and visual scores at least
as high as SVG within the same task/harness pair. Usage must be complete for
token comparisons. Three repetitions and subjective image ratings do not
establish general model, harness, or language rankings.

```sh
python3 -m unittest discover -s benchmarks/locus -p 'test*.py'
python3 benchmarks/locus/native_smoke.py --output target/locus-http-smoke-new
python3 benchmarks/locus/audit.py target/locus-skill-benchmark/provider-beta6-20261016-1 > target/historical-audit-v2-new.json
```

All three commands are offline/read-only with respect to provider services.
The tests need a built native CLI; PNG/native smoke cases need the locked Node
tools. The smoke uses the production coordinator, a localhost synthetic provider,
all three native harnesses and both workflows. It injects 21 forbidden tool
names, requires explicit denial responses, checks a sibling sentinel is neither
read nor modified, and checks no shell/patch output is created. It also verifies
HTTP checks, inspection, SVG/PNG and final export. On 2026-10-16 all six cases
passed, saved under `target/locus-http-native-smoke-verified/`. The 67-test offline
regression suite also passed. These are integration tests, not scored drawings
or provider-performance results.

The read-only v2 audit detects skill-path access without requiring another full
trial path, command-level producer violations, protected-input/evidence
references, forbidden native catalogs, and mismatched service producers. It
re-identifies the four known contaminated SVG trials and the two Locus-production
attempts in the historical archive. Flags describe references/attempts; successful
access/production must be corroborated with tool results. Quoted diagram text
such as `locus check` is not itself evidence of using Locus. The original audit
and ratings remain unchanged.

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
from the provider-metered historical harness above.

## Prepare and run

From the repository root, build the beta 6 CLI with `cargo build --release --locked
-p locus-cli`. A compatible Node executable and `sharp` module are required for
PNG previews. The default Sharp path reuses the historical local installation;
pass `--sharp /absolute/path/to/sharp` and `--node /absolute/path/to/node` when
needed. No preview dependencies are downloaded by the runner.

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
the protocol, tasks, templates, runner, tests, concise documentation and an
honest summary of actual observations.

## Offline verification

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/locus -p 'test_benchmark.py'
```
