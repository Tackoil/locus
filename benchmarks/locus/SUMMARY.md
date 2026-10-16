# Beta 6 skill and native SVG benchmark

## Completed DSH/Pi/OpenCode comparison: 2026-10-16

**Post-run correction:** [Failed-case analysis](FAILURE_ANALYSIS.md) found
workflow contamination: four SVG-assigned trials read the Locus skill, and
SVG flowchart `dsh-1` was rendered with Locus. The original access audit missed
these cases. The totals below preserve the assigned sessions; the SVG arm is
not a clean direct-SVG baseline, and its aggregate differences do not establish
a language comparison.

**All 54 trials finished. Locus-assigned sessions produced 20/27 scored exports;
SVG-assigned sessions produced 26/27. Locus used more reported tokens in every task/harness pair and
met none of the nine descriptive efficiency criteria.** Locus received higher
mean image scores for triangles and flowcharts; checkout sequences remained
unreliable and scored substantially worse.

| Task | Locus reported tokens | SVG-assigned reported tokens | Exports: Locus / SVG-assigned | Mean visual /100: Locus / SVG-assigned |
| --- | ---: | ---: | ---: | ---: |
| Triangle | 7,175,891 | 3,897,019 | 9/9 / 9/9 | 99.2 / 95.4 |
| Checkout sequence | 11,290,012* | 1,790,585 | 3/9 / 9/9 | 17.3 / 94.0 |
| Check-before-render flowchart | 15,702,606 | 5,372,717 | 8/9 / 8/9 | 86.1 / 81.1 |

*Four Locus sequence responses lacked provider usage. Their reported totals
are lower bounds; the DSH and Pi sequence token comparisons have incomplete
accounting. All SVG accounting is complete. Missing exports score zero, and
the means above include those failures. Scores are subjective assessments from
one independent reviewer per task, each comparing both workflows and all three
harnesses without sources or timing information.

The run recorded **45,228,830 reported tokens across 1,762 provider calls**,
zero HTTP failures, and seven authoring timeouts. Currency cost was not
measured. Locus authors directly left 19 SVG files; the coordinator generated
20 scored exports from passing final sources under the historical protocol.
Process success, native export, checked export, and image quality remain
separate measurements.

### Settings and review

Provider protocol 1.6 targets the final beta 6 skill. Native versions were DSH
0.1.5-rc.2, Pi 0.84.4, OpenCode 1.18.35, and Sharp 0.35.4. The model was
`deepseek-flash`, thinking disabled, temperature 0, with an 8,192-token response
cap. Each trial had a 64-provider-request budget and a 600-second authoring
timeout. Three tasks, two workflows, three harnesses, and three fresh
repetitions produced the 54 scheduled trials.

Three trials ran concurrently. The manifest correctly labels this a variation
from the historical sequential schedule (`canonical: false`). The PNG-enabled
debug beta 6 CLI retained full native request evidence. Elapsed time includes
instrumentation and final source checks/renders, which can extend beyond the
authoring timeout. These timings do not isolate CLI or solver performance.
Native tool instructions, vision capabilities, provider caching, and validation
overhead differ between workflows and harnesses.

All authors finished before image review began. Three fresh Codex reviewers,
one per task, inspected all **46 available exports** as anonymous raster images
and filled hash-bound 40/25/20/15 ratings. Eight missing outputs retain automatic
zero scores. Standard and full-detail previews were generated from unchanged
SVG bytes; reviewers received task text and images, without workflow mappings,
sources, metrics, or author reports. Rendering style can still reveal workflow.

### Observed failures and image findings

Four Locus sequence trials ended without the required final `figure.locus`,
leaving exploratory probe files instead. Two final sequence sources failed
joint layout with E413. All three exported Locus sequences came from timed-out
authoring trials; their previews still had serious defects, including detached
arrows, overlapping or poorly separated branches, and obscured lifelines or
activations. Native check success did not establish diagram correctness.

One Locus flowchart failed its own alignment assertion. One DSH SVG flowchart
used all 64 provider requests without creating its required SVG. These failures
and all their usage remain in the comparison. No drawing was repaired by the
coordinator, and no paid trial was replaced.

Among available images, reviewers found occasional malformed altitude-foot
marks and faint circles in triangles. Flowchart defects included a repair loop
returning to init instead of add, border overlap, and angular routes where a
curved return was requested. These findings explain why file validity and image
ratings are recorded separately.

### Evidence and validation

The final audit found **no material identity or accounting mismatches**.
Its access scan returned no flags, but subsequent manual analysis found reads
of the other workflow's skill and use of Locus in the SVG arm. That scan does
not establish isolation; see the correction above. Frozen/live input identities, software hashes, 1,762 request archives,
usage sums, exports, and summary/metric correspondence matched. Native evidence
includes 1,161 archived request records, with 287 failed invocations also retained
in the rejection archives, and 272 observed source snapshots. Capture types and repeated calls are distinguished;
these counts are not comparable authoring-retry measurements.

All 51 offline benchmark tests and native version/PNG preflight checks passed.
Maximum function cognitive complexity is 15, with no violations above 15. The
tested skill snapshot matches the final repository skill.

Saved under ignored `target/locus-skill-benchmark/provider-beta6-20261016-1/`:

- [Full paired comparison](../../target/locus-skill-benchmark/provider-beta6-20261016-1/report.md)
- [Protocol, settings, and frozen identities](../../target/locus-skill-benchmark/provider-beta6-20261016-1/manifest.json)
- [Evidence verification](../../target/locus-skill-benchmark/provider-beta6-20261016-1/verification.json)
- [Machine-readable grouped results](../../target/locus-skill-benchmark/provider-beta6-20261016-1/workflow-summary.json)
- [Image-review method and coverage](../../target/locus-skill-benchmark/provider-beta6-20261016-1/review/manifest.json)

Raw provider requests, usage, native CLI captures, probe/final sources, SVGs,
previews, ratings, and verification/review helpers remain in that directory.
The earlier Codex results below are a separate development experiment and are
not pooled with these historical-harness results.

## Completed development run: 2026-10-16

All six formal trials exported valid SVGs. The three Locus trials passed native
checks, strict layout checks, and deterministic source/export correspondence.
The three native SVG trials passed XML/namespace validation. An independent
reviewer inspected all six anonymized, freshly rasterized images after drawing
finished; visual scores are separate from parser and solver success.

| Task | Locus seconds | Native SVG seconds | Locus visual /100 | Native SVG visual /100 |
| --- | ---: | ---: | ---: | ---: |
| Triangle | 37 | 52 | 99 | 100 |
| Checkout sequence | 130 | 69 | 97 | 100 |
| Check-before-render flowchart | 85 | 62 | 98 | 100 |

Locus finished the triangle faster and the two diagrams more slowly in this
sample. Both workflows met the requested geometry and diagram semantics. The
reviewer deducted points from Locus for a prominent incircle style, smaller
sequence text and weaker hierarchy, and a flowchart spine left of the canvas
center. Native SVG received full rubric scores in these three images. These
subjective scores from one reviewer and one trial per task/workflow do not
establish general rankings.

Times are each agent's measured task interval, including its tool use,
verification, and preview inspection; they exclude parent dispatch, queueing,
later verification, and review. Provider tokens, cost, request counts, and
comparable revision counts were unavailable and remain null. The historical
token-efficiency criterion cannot be evaluated.

## What was rebuilt

The [skill](../../skills/locus/SKILL.md) follows the historical `skills/locus/`
structure, with preserved UI metadata and a rewritten portable reference. It
targets the current `locus 0 beta 6` contract: constructor declarations,
measured dimensions and labels, reusable functions, groups, automatic routes,
endpoint clipping, focused feedback, and safe edits. A runnable relational
routing example replaces the retired offset-based sample.

The prior skill and benchmark were introduced in `9af397d`, refined in
`b3e0fa4` and `873688c`, and removed by the beta 6 transition `bf323b7`.
Protocol 2.0 preserves the triangle, sequence, and flowchart task descriptions
byte-for-byte from `bf323b7^`. The new standard-library runner prepares isolated
workspaces, freezes inputs, validates untouched artifacts, and binds review
ratings to image hashes. It replaces the historical provider harness with
externally dispatched fresh Codex subagents for the development experiment
below. DSH, Pi, and OpenCode are restored separately as described above.

Each of the six formal authors started without parent conversation history.
They received their own saved prompt and workspace; only Locus authors received
the frozen skill. Workspace separation was instructed on a shared filesystem,
not enforced by an additional operating-system sandbox. The independent
reviewer received only opaque image IDs, task text, images, and rating fields;
sources, workflow mapping, timings, and author reports were withheld until
scoring finished. Rendering style can still reveal the workflow.

The tested skill snapshot matches the final repository skill. Runner, prompts,
protocol, tool identities, source hashes, and image hashes were verified. No
coordinator repaired a drawing, and all six scheduled formal trials are retained.

## Retained evidence and authoring friction

The sequence agent retained five intermediate Locus sources and diagnostics.
Its early mistakes included inline `opacity`, annotation anchors used where
geometric points were required, and a forward reference to the alt frame.
The flowchart agent retained an invalid alignment assertion and a spacing
change that caused a layout conflict. Both repaired their own work and ended
with clean strict layout checks. These retained attempts are evidence of
authoring friction, not comparable API retry or revision measurements.

Two preliminary attempts are excluded and preserved separately: a native SVG
triangle using an earlier wrapper prompt, and one Locus triangle started before
the routing sample's final caption-clearance correction. The latter experiment's
other five trials were never dispatched. Neither pilot contributes to the table.

Generated evidence is under ignored
`target/locus-skill-benchmark/beta6-20261016-codex-final/`:

- [Full comparison report](../../target/locus-skill-benchmark/beta6-20261016-codex-final/report.md)
- [Frozen manifest and identities](../../target/locus-skill-benchmark/beta6-20261016-codex-final/manifest.json)
- [Dispatch and timing provenance](../../target/locus-skill-benchmark/beta6-20261016-codex-final/execution.json)
- [Image-specific ratings](../../target/locus-skill-benchmark/beta6-20261016-codex-final/review/ratings.json)

Trial folders retain prompts, source, SVGs, PNG previews, diagnostics, native
verification logs, and immutable metrics. The repository contains the reusable
skill, runner, protocol, tasks, tests, and this summary; generated runs stay local.

## Validation

The skill validator passed. All three reference snippets and the runnable routing
sample passed strict layout checks; the sample rendered to SVG, PNG, and TikZ.
The rebuilt PNG-enabled release CLI was used for the formal trials.

All 16 offline benchmark tests passed, covering evidence drift, immutable
records, failures and timeouts, native source/export matching, review rendering,
and invalid or stale ratings. Cognitive complexity passed with maximum function
score 15 and no violations above 15. Frozen input identity, final skill identity,
review completeness, and whitespace checks passed.

See [README.md](README.md) for reproduction commands and interpretation limits.
