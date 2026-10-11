# Locus benchmark request analysis

The 2026-10-11 protocol 1.5 run shows that excess reported tokens came mainly
from repeated repair loops and growing input history. The strongest improvement
opportunities are actionable layout feedback, complete diagram examples, and a
bounded authoring workflow. The benchmark wrapper also inflated check feedback
by discarding the requested compact output.

This analysis covers `target/locus-skill-benchmark/run-20261011-3`. It describes
the frozen inputs and installed executable used by that experiment, rather than
every feature in the latest checkout. Later commit `1add60e` added collision
details and layout checks; those improvements were not measured in this run.
See [SUMMARY.md](SUMMARY.md) for the comparison and earlier experiments.

## Evidence and measurement limits

The verified capture contains 1,230 completed native CLI records, including
harness and coordinator checks: 1,052 successes and 178 failures. It retains
446 distinct full source snapshots, 163 final workspace source copies, and
493 shell tool requests mentioning Locus commands. These counts do not represent
1,230 agent decisions or 178 distinct engine defects.

| Workflow | Reported input tokens | Reported output tokens | Total tokens | Provider calls | Exported images |
| --- | ---: | ---: | ---: | ---: | ---: |
| Locus | 38,814,703 | 333,360 | 39,148,063 | 1,025 | 25/27 |
| Direct SVG | 6,046,756 | 126,627 | 6,173,383 | 433 | 25/27 |

Input tokens account for 99.15% of Locus's total. Earlier instructions, source
and tool results recur in subsequent requests. Reported tokens include cached
input and are not a billing comparison. Even excluding the largest failed Locus
trial, its remaining total is 26,898,460 tokens. This exclusion is a sensitivity
calculation, not a replacement benchmark result.

The overall comparison is contaminated. The coordinator saved intermediate
image reviews inside the experiment directory while trials were running; direct
SVG sequence DSH-3 read them repeatedly. Direct SVG flowchart DSH-2 read frozen
benchmark implementation and tests. Both failed to produce drawings. Native
filesystem permissions allowed these reads. The run therefore supports local
failure analysis but cannot establish a clean causal Locus versus SVG result.

## Causes observed in requests and sources

### Probing replaced the deliverable

Locus flowchart DSH-2 used 12,249,603 tokens, or 31.29% of all Locus tokens,
and reached the 64-request budget without creating `figure.locus`. It produced
57 distinct probe sources. Its 53 failed CLI records deliberately included
contradictory assertions such as a distance equal to 99, then 98, then 97.
Those rejections were expected. The retained tool history reached approximately
1.54 million characters, with individual tool outputs reaching the harness's
50,000-character limit.

The skill needs a saved draft before experiments, a small repair budget, and
a stopping rule when repeated probes do not advance the requested image.
Assertions verifying every derived offset add unnecessary work; the existing
skill already advises against them, but this trial did not follow that advice.

### The wrapper discarded compact output

The frozen `inputs/benchmark/validation.py` wrapper intercepted checks of the
task source, called `validate(binary, source)`, and printed the resulting full
report. It did not forward `--compact`, or other check options, to that call.
Agents requesting compact feedback therefore received full solver and assertion
reports. The wrapper also checked the source before each render; coordinator
observations added further native checks.

Replaying the final sequence OpenCode-2 source directly with the experiment's
installed executable produced 4,326 bytes for `check --json` and 378 bytes for
`check --json --compact`. This establishes the output difference for that source;
it does not quantify total token savings from fixing the wrapper.

### Collision feedback encouraged blind repair loops

Sequence Pi-2 accumulated 113 captured render requests while investigating
warnings such as `labels collide: apprLabel`. In the measured CLI, a singleton
warning could represent an intersection with geometry or a margin violation,
but it named only the label. The agent removed lifelines, tested object subsets,
changed label directions and inspected SVG outline paths to guess the cause.
Sequence OpenCode-2 also investigated collisions through whole-scene descriptions
and SVG path parsing.

Geometry participants, collision type, and relevant bounds make a repair
actionable. The recorded guessing does not by itself establish that collision
detection was incorrect. Later commit `1add60e` addresses collision reporting
and adds layout checks; a fresh benchmark is needed to measure the effect.

### Measurement queries rejected language anchors

There were 14 captured `inspect_query` failures. Agents requested measurements
using anchors such as `customer.center`, `customerLife.start`, and `altFrame.nw`,
which are valid in language expressions but were rejected by CLI point queries.
A direct replay of `inspect --distance customer.center payment.center` returned
exit 3 with `unknown point customer.center`.

CLI measurements should resolve geometry anchors consistently with language
measurements. Until then, the skill should explain the limitation and show a
named derived point as a query operand. Targeted object descriptions would also
avoid returning a whole scene to investigate one object.

### Valid sources still produced incorrect diagrams

Sequence OpenCode-2 passed its authored assertions but scored 69/100 visually.
Its activation bars ended before `charge`, its alternative frame enclosed
common requests and missed later declined messages, and its divider extended
beyond the frame. The source projected `dividerRow` onto the horizontal segment
`seg(altFrame.w, altFrame.e)`. That projection loses the requested vertical row
position. Assertions covered offsets, horizontal messages and incidence rather
than branch containment or processing duration.

Six of eight exported Locus flowcharts omitted the required title. Several used
angular return paths where a curved connector was requested. Anchored
`text_block` titles and captions also produced visible leader tails. A complete
example should show the intended annotation behavior as well as routing.

The measured `check --strict` passed the final sequence OpenCode-2 source without
layout warnings: layout diagnostics were generated during rendering. A passing
check certified its implemented conditions, not the requested diagram semantics.
The skill already required visual review; turning that instruction into a
concrete task checklist would make omissions harder to overlook.

### Source generation reached the output limit

Sequence DSH-2 made six provider requests and saved no source. Its final response
used exactly 8,192 completion tokens, the configured output limit; no subsequent
source-writing tool execution was recorded. This is consistent with generation
being cut off before a usable tool call. Short complete examples and writing
source in manageable sections can reduce this risk.

### Common syntax repairs were avoidable

Repeated errors included specifying both `place` and `anchor` on a `text_block`,
redeclaring the built-in `highlight` class, and applying point-only `mark: none`
to a line. Some unknown-reference errors came from temporary probes that deleted
declarations while retaining their dependents. The capture contains no `E403`
solver-convergence errors; it does not support making a solver rewrite the main
response to this experiment.

## Improvement priorities

| Priority | Area | Improvement | Evidence addressed |
| --- | --- | --- | --- |
| 1 | CLI | Identify collision participants and type, expose useful bounds, and offer an explicit layout check. | Long collision investigations; check/render feedback differed. Collision details and layout checks were subsequently added in `1add60e`. |
| 2 | Skill | Supply complete runnable sequence and flowchart examples with branch frames, dividers, activation spans, curved returns, titles and captions. Derive frame and activation boundaries from message rows. | Passing sources still had incorrect containment, duration, routing and missing text. |
| 3 | Skill | Save a draft early, diagnose one issue at a time, preserve dependency closure, stop ineffective repeated probes, and review a requirement checklist. | The largest trial never created the requested source; deletion probes created unrelated errors. |
| 4 | CLI | Accept geometry anchors in measurement queries and add targeted `describe --id` output. | Fourteen rejected queries and unnecessarily broad scene reports. |
| 5 | CLI and skill | Explain mutually exclusive placement properties, reserved class names and point-only properties. Bound repetitive diagnostic details with explicit counts and optional expansion. | Recurring syntax repairs and large histories of failed assertions. |

Before another paid run, preserve CLI options through the benchmark wrapper
and keep reviewer material outside active agents' accessible inputs. Record
these behavior changes in the next protocol version. A subsequent run should
measure whether repairs, feedback size and total reported tokens actually fall.

## Retained artifacts

Generated evidence remains locally under ignored
`target/locus-skill-benchmark/run-20261011-3/`. The committed summary and this
analysis remain readable without those local artifacts; the detailed evidence
links require the original experiment directory.

- [Full CLI requests and source bundle](../../target/locus-skill-benchmark/run-20261011-3/locus-cli-requests.zip)
- [Request and source index](../../target/locus-skill-benchmark/run-20261011-3/cli-requests/index.json)
- [Computed request statistics](../../target/locus-skill-benchmark/run-20261011-3/request-analysis.json)
- [Comparison report](../../target/locus-skill-benchmark/run-20261011-3/report.md)
- [Visual observations](../../target/locus-skill-benchmark/run-20261011-3/image-reviews.json)
- [Verification results](../../target/locus-skill-benchmark/run-20261011-3/verification.json)
- [Full contradictory probe source](../../target/locus-skill-benchmark/run-20261011-3/cli-requests/sources/10af4a1b0b2f1ee0de6bc82e39f67343b9a1265eefdb94657b2abeb4f7b210ec.locus)
- [Sequence source with frame and activation errors](../../target/locus-skill-benchmark/run-20261011-3/locus/sequence/opencode-2/figure.locus)
