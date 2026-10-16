# Beta 6 failed-case analysis

Analysis of `provider-beta6-20261016-1`, completed 2026-10-16. This uses the
existing 54 trials, archived tool conversations, final sources, diagnostics,
and image ratings. Additional local CLI probes are separate from the benchmark;
no provider calls, trial repairs, replacement trials, or score changes were made.

## Findings

The eight missing scored exports have different causes: four sequence trials
never delivered the required source, two delivered inconsistent sequence
constraints, one flowchart failed an alignment assertion, and one SVG-assigned
trial switched to Locus and never delivered SVG. All three exported Locus
sequences also had material visual or completeness defects.

There is a separate, reproducible solver convergence defect in a feasible
point-on-segment construction. Consequently, E413 cannot be attributed to
author mistakes in general, even though several failed sources contain direct
contradictions. Instrumentation also adds repeated solves to Locus authoring
time. The run does not isolate engine speed.

**The comparison has workflow contamination.** Four SVG-assigned trials read
the Locus skill, and the scored SVG flowchart `dsh-1` was actually rendered by
Locus. The original audit's zero access flags missed these cases. Recorded
totals and images remain useful evidence of the assigned sessions, but the
aggregate SVG arm is not a clean direct-SVG baseline.

## Eight missing scored exports

Paths below are relative to the [saved run](../../target/locus-skill-benchmark/provider-beta6-20261016-1/).
`raw/<trial>/requests/` contains cumulative provider conversations;
`raw/<trial>/work/` retains author files.

| Assigned workflow / task / trial | Observed outcome | Cause supported by evidence |
| --- | --- | --- |
| Locus / sequence / `dsh-2` | 600-second authoring timeout; probes through `probe25.locus`, no final `figure.locus` | Exploration grows from passing small examples into contradictory clearance requirements. `probe19.locus:15` centers an activation bar on B, an endpoint of the message paths, then lines 75–78 require the bar to avoid those paths. Later repairs add more constraints rather than remove the conflict. Archived results include 120-second tool timeouts. |
| Locus / sequence / `dsh-3` | 600-second authoring timeout; only a probe source | `probe.locus:22–23` centers `act` on `b.shape`; lines 25 and 30 then require avoidance of its centered caption and the shape itself. Lines 28–40 repeatedly broaden avoidance. This is impossible for positive-size rectangles sharing a center. |
| Locus / sequence / `opencode-1` | 64 provider requests; no final source | `probe.locus:13` creates an anchored label, then line 23 passes it to measured containment. E404 says “expected an object with measured bounds.” The last archived conversation contains **41 identical tool calls** rewriting/checking the same probe. This is a repair loop without progress. The probe also contains unrelated conflicting bar-center/separation requirements. |
| Locus / sequence / `opencode-2` | 64 provider requests; only four participant headers | The final probe is a 17-line header row. The last archived conversation contains **26 identical calls** printing the same SVG path attributes. No sequence messages, frame, or required final source were delivered. This is an inspection loop, without evidence that final construction was attempted. |
| Locus / sequence / `pi-2` | Authoring timeout; final source E413 | In `figure.locus:36`, the lifeline bottom is 40 units below its header. L1 is 3 below the header, but `vdist(L2,L1) == -3` and subsequent negative values put later levels progressively **above** L1. L3–L11 lie above the lifeline start while message endpoints must be `on` its finite segment. The two self-calls also use coincident endpoints. The agent notices the latter near the end but does not leave a corrected source. |
| Locus / sequence / `pi-3` | Authoring timeout; final source E413 | Multiple clauses require both equal x coordinates and a nonzero x displacement. For example line 73 combines `align_center_x(alt_nw,lc_top)` with `hdist(lc_top,alt_nw) == -1.5`; line 94 repeats the pattern with `-0.25`. The frame corners, guards, message endpoints, and legend contain this conflict. |
| Locus / flowchart / `dsh-1` | 64 requests; solve succeeds, final assertion fails | `figure.locus:32` uses `gap_y(decide,fix.shape)` despite the comment requiring the same row. Line 80 asserts `align_center_y` without defining it. The solver satisfies the vertical gap; the assertion correctly rejects the resulting layout. An assertion checks a result and does not impose a relation. |
| SVG / flowchart / `dsh-3` | 64 requests; `figure.locus` exists, `figure.svg` absent | After reading the other workflow's skill, the agent searches for the Locus binary, writes Locus, and spends the remaining budget on Locus probes. The required direct-SVG workflow was abandoned. This is an instruction-following and isolation failure, not evidence of an inability to author SVG XML. |

Evidence: [Locus sequence raw files](../../target/locus-skill-benchmark/provider-beta6-20261016-1/locus/sequence/raw/),
[flowchart source and assertion](../../target/locus-skill-benchmark/provider-beta6-20261016-1/locus/flowchart/dsh-1/figure.locus),
[flowchart check result](../../target/locus-skill-benchmark/provider-beta6-20261016-1/locus/flowchart/raw/dsh-1/final-check.json),
[SVG-assigned failed trial](../../target/locus-skill-benchmark/provider-beta6-20261016-1/svg/flowchart/raw/dsh-3/).

## Passing checks with poor diagrams

The three exported Locus sequences all exceeded the authoring deadline.
Their final source checks pass, but they do not fully encode the requested
sequence semantics or visual requirements.

| Trial | Image score | Source and image findings |
| --- | ---: | --- |
| `dsh-1` | 18/100 | Positive y displacements make time run upward. Consecutive message endpoints have different heights and are assigned across the wrong participant columns, creating detached diagonal arrows. The lifeline is only 0.6 units long. Self-calls are straight arrows; the alt frame does not enclose its messages; the title overlaps headers. The final payment-error message and legend are absent. |
| `pi-1` | 56/100 | Messages and self-call loops are substantially correct, but the layout is excessively tall. Both guard labels occupy the same upper frame area, with no branch divider. Activation bars and a request/return legend are absent. Merely containing the messages does not establish two mutually exclusive operands. |
| `opencode-3` | 82/100 | A recognizable sequence with activations and a divider. Guard text collides with the alt label or a lifeline; title and legend are missing; visible construction-point dots add clutter. This is mainly incomplete finishing and annotation placement. |

See [the sequence ratings](../../target/locus-skill-benchmark/provider-beta6-20261016-1/locus/sequence/ratings.json)
and [the lowest-scoring preview](../../target/locus-skill-benchmark/provider-beta6-20261016-1/locus/sequence/raw/dsh-1/preview.png).

The native-SVG images have independent defects. Sequence `dsh-1` scores 84:
opaque branch backgrounds cover lifelines and activations because of drawing
order. Flowchart `dsh-2` scores 84: its repair edge returns to **init**, rather
than **add**, and is angular. OpenCode flowcharts 1 and 2 overlap the add-node
border with their final return segment. Triangle OpenCode 1/3 and DSH 3 have
malformed or detached H right-angle marks despite correct main triangle geometry.
These are semantic, layering, and local construction errors that XML validity
cannot detect. [SVG ratings by task](../../target/locus-skill-benchmark/provider-beta6-20261016-1/svg/)
retain all observations.

## A feasible construction that beta 6 rejects

A reduced probe, using the **exact benchmark CLI binary**, returns E403:

```locus
locus 0 beta 6
style textbook
point A()
point B() where { align_center_x(A, B), vdist(B, A) == 40 }
segment life(A, B)
point L() where { vdist(L, A) == 3 }
point N() where { on(N, life), align_center_y(L, N) }
```

A witness is A=(0,0), B=(0,-40), L=N=(0,-3). Every stated relation holds,
including membership in the finite segment. The CLI instead reaches its LM
damping ceiling, with maximum reported residual about 0.00429. Release and
debug builds reproduce the same failure. This demonstrates a convergence
defect for a feasible system; it does not establish how many full benchmark
failures would remain after correcting their authored contradictions.

The seven reduced release probes also confirm that equal-x plus nonzero-x
displacement fails, removing equal-x passes, anchored-label containment raises
E404, replacing it with a standalone measured label passes, and centered
rectangles required to avoid one another fail. All probes finished within
their eight-second limits. They are demonstrations, not replacement scores.

Evidence: [reduced source](../../target/locus-failure-analysis-20261016/below-segment-level.locus),
[exact benchmark binary result](../../target/locus-failure-analysis-20261016/debug-feasible-segment.json),
[all local probe results](../../target/locus-failure-analysis-20261016/results.json).

## Workflow contamination and audit correction

Confirmed reads of the frozen Locus skill in the SVG arm:

| SVG-assigned trial | Archived evidence | Consequence |
| --- | --- | --- |
| flowchart `dsh-1` | `requests/0005.json` onward; `0062.json` contains the result of rendering `figure.locus` to `figure.svg` | The scored 96/100 image was produced using Locus and the repository release binary, rather than direct SVG XML. Its usage is included in the assigned SVG arm. |
| flowchart `dsh-3` | `requests/0005.json`, `0039.json`, `0040.json` | Switched to Locus; no SVG deliverable. |
| flowchart `pi-3` | `requests/0009.json` contains the skill read result | Skill exposure; the final deliverable is directly authored SVG. |
| triangle `dsh-2` | `requests/0009.json` and `0010.json` | Skill/reference exposure; the final deliverable is generated as SVG XML. |

Other SVG trials inspect benchmark inputs beyond the supplied renderer,
including validation code and workflow templates. The evidence supports broader
discovery outside the instructed workspace; it does not establish that agents
read another trial's drawing source or review ratings. Parent-directory
listings alone should not be described as reading another drawing.

The SVG prompt also retains the generic phrase “supplied skill,” although its
workflow instruction explicitly requires SVG XML. Accessible shared inputs
and this wording make skill discovery ambiguous. Rendering the SVG-assigned
deliverable with Locus still directly violates the specified workflow.

The old audit checks paths naming a different complete trial, other experiment
names, ratings, reports, metrics, and credentials. It does **not** flag access to
the sibling `inputs/skill` directory, parent-directory discovery, or use of the
wrong drawing tool. Its identity/hash/accounting checks remain useful, but its
zero access flags do not establish workflow isolation. This analysis supersedes
that interpretation. Original audit output is preserved.

Keep the recorded intention-to-treat totals, explicitly label noncompliance,
and do not silently exclude these trials or claim a clean language comparison.
A future direct-SVG baseline needs an isolated input directory and an audit of
the tools actually used. A token filter or broader regex alone is insufficient
to enforce filesystem isolation.

## Budgets, usage, and timing

Five further budget exits still have useful scored exports: Locus triangle
`dsh-1`/`dsh-3` (both 100), Locus flowchart `dsh-2` (95) and `opencode-3` (97),
and SVG-assigned flowchart `dsh-1` (96, workflow violation described above).
Locus flowchart `dsh-2` was exported by the coordinator from its passing final
source. Process failure must not be equated with missing output. Together with
the eight missing exports and three unfinished exported sequences, these are
the 16 sessions with a timeout or nonzero author exit.

Reported Locus usage is 33,825,033 prompt tokens and 343,476 completion tokens;
SVG-assigned usage is 10,810,433 prompt and 249,888 completion tokens. About 99%
of reported Locus tokens are prompt tokens. Cumulative histories, repeated
diagnostics, and tool loops are a substantial contributor; this is not a
measurement of source-code verbosity. Provider-reported prompt-cache hits are
32,836,864 and 10,441,216 respectively, so token multipliers must not be called
currency-cost multipliers. Four Locus sequence responses lack usage, and actual
currency cost remains unmeasured.

The Locus wrapper performs a check for expected-source check/render invocations;
render then solves again. The proxy observer also synchronously repeats checks
before forwarding the next provider request. That observer time counts toward
the author deadline but is excluded from the provider request's recorded
duration. Final coordinator validation/rendering is outside the author timeout.
The seven timeouts all occur in Locus sequences, but current timings cannot
separate agent work, duplicate validation, debug-build overhead, and engine
performance. No provider HTTP failure explains these cases.

The wrapper also discards requested check options: `probe()` calls
`validate(binary, source)`, which invokes only `check <source> --json`.
An author's `--layout --strict` invocation through that wrapper therefore
receives ordinary-check results. This weakens validation feedback and makes
such tool output insufficient evidence of a strict layout pass. The separate
local probes above invoke the real binary directly. See
[the wrapper](validation.py) and [proxy observer](meter.py).

## Recommended order of work

1. **Repair the benchmark boundary and audit before another comparative run.**
   Give SVG trials only their own workspace and renderer, separate workflow
   inputs, record actual producer tools, and flag workflow noncompliance. Add
   offline cases for skill reads and Locus-generated SVG in the SVG arm. Preserve
   this run as exploratory evidence.
2. **Fix the feasible segment-convergence regression.** Promote the reduced
   satisfiable example to an engine regression test. Improve diagnostics so
   a convergence failure does not imply that authored requirements are
   necessarily inconsistent.
3. **Add a small runnable sequence recipe to the skill.** Demonstrate y-up
   downward ordering, horizontal message endpoints on the correct participants,
   distinct self-call endpoints, branch divider/guards, activation intervals,
   title, and legend. The existing complete example covers a flowchart; a syntax
   catalog alone leaves agents assembling the hardest diagram from probes.
4. **Teach repair by removing the conflicting fact.** Include equal-x versus
   displacement, center versus avoidance, `gap_y` versus same-row alignment,
   and standalone measured versus anchored labels. Stop identical-call loops
   and retain a minimal complete final source before elaborate polishing.
5. **Measure authoring and validation separately.** Avoid synchronous duplicate
   solves in request observation, preserve requested check flags in wrappers,
   bound validation time, and record tool/observer/final-export durations.
   Retain visual review and task-semantic checks: a passing solver or XML parser
   cannot verify message ownership, branch alternatives, return targets, or
   visible layering.

These recommendations were not applied during this analysis. The skill,
harness implementation, frozen inputs, and scored trial artifacts are unchanged.
