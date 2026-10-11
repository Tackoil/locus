# Locus benchmark summary

## Latest rerun: 2026-10-11, protocol 1.5, run 3

[Request analysis and improvement priorities](REQUEST_ANALYSIS.md) traces the
captured CLI requests and full source snapshots, including repair loops,
feedback inflation, diagram errors and the experiment's measurement limits.

**All 54 trials completed, but this comparison is contaminated by exposure to an interim reviewer file.** The coordinator saved image-review checkpoints inside the experiment directory while trials were running. Direct-SVG sequence DSH run 3 read that file repeatedly and exhausted its request budget without creating a drawing. Direct-SVG flowchart DSH run 2 also inspected frozen benchmark implementation and tests before failing to create its drawing. Native filesystem permissions allowed these reads. The checkpoint should have remained outside running agents' accessible inputs until every trial ended. Full commands and provider history are retained in `context-exposure.json` and the raw logs. These results describe the recorded run and should not be treated as a clean controlled comparison of Locus versus SVG.

| Task | Workflow | Reported tokens | SVGs | Mean visual /100 | Locus/SVG token ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| triangle | locus | 4,930,501 | 9/9 | 95.2 | 4.09× |
| triangle | svg | 1,204,102 | 9/9 | 80.0 | — |
| sequence | locus | 15,732,626 | 8/9 | 79.0 | 6.94× |
| sequence | svg | 2,267,047 | 8/9 | 87.1 | — |
| flowchart | locus | 18,484,936 | 8/9 | 80.4 | 6.84× |
| flowchart | svg | 2,702,234 | 8/9 | 82.6 | — |

Both workflows exported **25/27** images. All 50 available images were inspected using hash-bound 40/25/20/15 ratings; four missing exports score zero. Missing outputs were Locus sequence DSH run 2 and flowchart DSH run 2, plus direct-SVG flowchart DSH run 2 and sequence DSH run 3. No failed trial was replaced and no drawing was repaired. The run met none of the nine paired descriptive criteria.

The current checkout was rebuilt and installed as a PNG-enabled debug executable at `target/locus-cli-install/bin/locus`. No `cargo run` shell requests were observed. Canonical protocol settings and pinned harness versions were retained. Reported usage was **45,321,446 tokens across 1,458 provider calls**, with complete accounting, zero provider failures and zero timeouts.

The verified bundle contains **1,230 completed CLI records** (1,052 successes, 178 failures), **446 distinct full source snapshots**, **163 final workspace source copies**, complete per-trial `.locus` request/rejection directories, and **493 shell tool requests mentioning Locus commands**. Records retain arguments, working directory, available original/candidate/evaluated source contents, stdin, diagnostics and results. Source copies are deduplicated by SHA-256 while invocation records remain separate.

Frozen and live inputs, installed executable and harness hashes, image hashes, copied request/rejection records, original/candidate/evaluated/stdin bytes, workspace source copies, and every ZIP entry were verified. All **43 offline tests** passed (34 benchmark, 9 CLI request-capture).

Saved under ignored `target/locus-skill-benchmark/run-20261011-3/`: [comparison report](../../target/locus-skill-benchmark/run-20261011-3/report.md), [CLI requests and full source copies](../../target/locus-skill-benchmark/run-20261011-3/locus-cli-requests.zip), [request index](../../target/locus-skill-benchmark/run-20261011-3/cli-requests/index.json), and [verification results](../../target/locus-skill-benchmark/run-20261011-3/verification.json). Raw provider requests, logs, frozen inputs, ratings and images remain in the experiment directory.

## Full request-capture rerun: 2026-10-11, protocol 1.5

**All 54 trials completed. Locus met none of the nine paired effort/quality criteria and used more tokens in every task/harness pair.** Triangle quality was higher with Locus; sequence and flowchart quality and export reliability were lower.

| Task | Workflow | Reported tokens | SVGs | Mean visual /100 | Locus/SVG token ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| triangle | locus | 4,153,249 | 9/9 | 97.6 | 2.05× |
| triangle | svg | 2,021,483 | 9/9 | 79.3 | — |
| sequence | locus | 19,595,693 | 5/9 | 48.9 | 5.27× |
| sequence | svg | 3,720,125 | 9/9 | 99.4 | — |
| flowchart | locus | 10,335,972 | 8/9 | 78.8 | 6.53× |
| flowchart | svg | 1,582,360 | 9/9 | 94.3 | — |

Locus exported **22/27** scored images; direct SVG exported **27/27**. All 49 available images were inspected using the hash-bound 40/25/20/15 rubric. Missing exports score zero. No failed trial was replaced and no model-authored drawing was repaired. The five missing Locus exports were sequence DSH runs 1/2, Pi run 1, OpenCode run 1, and flowchart DSH run 2. There were no timeouts.

All Locus triangles contained the requested construction, although extended hypotenuses and detached labels reduced readability in some. Direct SVG triangles sometimes hid the incircle, misplaced the H angle mark, or clipped circles; OpenCode run 1 rendered with enormous strokes that obscured the drawing. Locus sequences sometimes lacked branch dividers or correctly placed activation bars and branch labels. Seven of the eight exported Locus flowcharts omitted the title; several repair loops were angular, detached, or had ambiguous arrowheads. The final Pi Locus flowchart included the title and a clear curved loop.

The run used a cleanly rebuilt, installed PNG-enabled debug CLI from the current checkout at `target/locus-cli-install/bin/locus`; its SHA-256 is recorded in the manifest. No `cargo run` shell requests were observed. The canonical protocol retained the pinned harness versions, DeepSeek Flash, disabled thinking, temperature 0, three repetitions, 64 provider requests and 600 seconds per trial, with interleaved workflows and rotated harness order. Total reported usage was **41,408,882 tokens across 1,528 provider calls**, with complete usage and no recorded provider failures.

The verified request bundle contains **1,039 completed CLI records** (903 successes, 136 failures), **301 distinct lossless source snapshots**, **189 complete final workspace source files**, and **473 shell tool requests mentioning Locus commands**. Records retain arguments, working directory, full available original/candidate/evaluated sources, stdin, diagnostics and results. Source copies are deduplicated by SHA-256; invocation records remain separate. Help/version records and the rejection archives are included. Shell requests and workspace copies supplement native records for commands that might end before saving a completed result.

Frozen/live inputs, installed executable and harness command hashes, image-rating hashes, copied records, every retained source copy and ZIP integrity were verified. Offline checks passed **43 tests** (34 benchmark and 9 CLI request-capture tests). A sandbox-blocked startup under `run-20261011-1/` made no provider calls and is preserved separately. The completed experiment is the canonical run below. These are descriptive results from three repetitions and one reviewer, comparing full authoring workflows.

Saved locally under ignored `target/locus-skill-benchmark/run-20261011-2/`: [comparison report](../../target/locus-skill-benchmark/run-20261011-2/report.md), [CLI requests and full source copies](../../target/locus-skill-benchmark/run-20261011-2/locus-cli-requests.zip), [request index](../../target/locus-skill-benchmark/run-20261011-2/cli-requests/index.json), and [verification results](../../target/locus-skill-benchmark/run-20261011-2/verification.json). Raw provider requests, native logs, frozen inputs, ratings and images remain in the experiment directory.

## Full request-capture rerun: 2026-10-10, protocol 1.5

**All 54 trials completed. Locus met none of the nine paired effort/quality criteria.** It used more reported tokens in eight pairs; the lower-token DSH flowchart pair produced two blank Locus exports and one missing export. Locus triangle quality improved overall, while sequence and flowchart quality were lower.

| Task | Workflow | Reported tokens | SVGs | Mean visual /100 | Locus/SVG token ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| triangle | locus | 6,887,098 | 9/9 | 97.8 | 5.64× |
| triangle | svg | 1,221,526 | 9/9 | 87.4 | — |
| sequence | locus | 20,299,876 | 7/9 | 67.9 | 13.46× |
| sequence | svg | 1,508,080 | 9/9 | 99.4 | — |
| flowchart | locus | 14,000,940 | 8/9 | 63.6 | 2.79× |
| flowchart | svg | 5,016,197 | 7/9 | 70.1 | — |

Locus exported **24/27** images, including two blank coordinator exports; direct SVG exported **25/27**. All 49 available images were inspected using the 40/25/20/15 rubric and hash-bound ratings. Blank and missing images score zero. No model output was repaired or failed trial replaced. Locus sequence Pi run 1 timed out but retained an exportable source; elapsed time includes coordinator checks and rendering beyond the native 600-second limit.

The run used the installed PNG-enabled debug executable at `target/locus-cli-install/bin/locus`, with no observed `cargo run` shell requests. Protocol 1.5 uses three shared task descriptions and two workflow templates, with the installed-executable rule in both templates. DeepSeek Flash, disabled thinking, temperature 0, the pinned harness versions, three repetitions and the 64-request budget were retained. The run recorded **48,933,717 provider tokens across 1,716 provider requests**, with complete reported usage and no recorded provider failures.

The request bundle contains **1,517 completed CLI records** (1,338 successes and 179 failures), **601 distinct lossless `.locus` source snapshots**, **234 final workspace source files**, and **651 shell tool requests mentioning Locus commands**. Each completed record retains arguments, working directory, full available input/candidate/evaluated sources, stdin, diagnostics and results; help/version requests are included. Interrupted commands can lack completed native records, so their shell requests and final workspace sources are also retained. Source snapshots are deduplicated by SHA-256; requests remain separate.

Frozen and live input hashes, the installed executable, harness command files, image-rating hashes, copied records, source bytes and ZIP integrity were verified. Offline checks passed **43 tests** (34 benchmark and 9 CLI request-capture tests); maximum function cognitive complexity was 14 against a threshold of 15. This is a descriptive comparison of complete authoring workflows with three repetitions and one reviewer, not a measurement of human usability or an isolated language effect.

Saved locally under ignored `target/locus-skill-benchmark/run-20261010-2/`: [comparison report](../../target/locus-skill-benchmark/run-20261010-2/report.md), [CLI requests and full source copies](../../target/locus-skill-benchmark/run-20261010-2/locus-cli-requests.zip), [request index](../../target/locus-skill-benchmark/run-20261010-2/cli-requests/index.json), and [verification results](../../target/locus-skill-benchmark/run-20261010-2/verification.json). Raw provider requests, native logs, frozen inputs, ratings and rendered images remain in the experiment directory.

## Workspace-search rerun: 2026-10-08, protocol 1.2

**All 54 fresh trials completed, with no timeouts. These trials do not demonstrate reduced drawing effort with Locus.** Locus used more reported tokens in all nine task/harness pairs and met none of the protocol's paired effort/quality criteria. Triangle geometry remained correct, with a small overall visual gain; sequence and flowchart export reliability and mean visual quality were lower.

| Task | Workflow | Reported tokens | SVGs | Mean visual /100 | Locus/SVG token ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| triangle | locus | 4,312,902 | 9/9 | 90.1 | 2.02× |
| triangle | svg | 2,130,679 | 9/9 | 88.1 | — |
| sequence | locus | 16,999,137 | 5/9 | 37.7 | 17.25× |
| sequence | svg | 985,712 | 8/9 | 85.3 | — |
| flowchart | locus | 16,204,553 | 7/9 | 74.3 | 29.60× |
| flowchart | svg | 547,521 | 9/9 | 92.7 | — |

Locus exported **21/27** scored images, including three coordinator exports from checked sequence sources; direct SVG exported **26/27**. Codex inspected all 47 available exports using the 40/25/20/15 rubric. Ratings are bound to SVG hashes; missing exports score zero. No failed trials were replaced or model artifacts repaired by the reviewer.

### Search guards and comparability

Protocol 1.2 adds workspace-only guards for conventional filesystem search/listing commands and an explicit Pi tool-call hook. Locus prompts identify the supplied reference paths and state that the full specification is absent. Model settings, pinned native harness versions, three repetitions, 64-call budget and 600-second timeout remain as described in [README.md](README.md). This is a new protocol comparison, not a controlled estimate of the guard's isolated effect.

The Pi logs contain **68 blocked tool-call records**. These include out-of-workspace paths and conservative batch rejections: a workspace search/listing can be rejected when another command in the batch includes an external path or shell variable, including the supplied external SVG renderer. This can add authoring friction. The guard is not an operating-system sandbox; DSH/OpenCode's PATH wrappers can be bypassed by absolute executables, and arbitrary programs are not fully confined. See the README for the precise scope.

Every trial used the frozen starting skill and benchmark inputs. Frozen hashes, live benchmark-code hashes, Locus executable hash and harness command-file hashes were verified after completion. The live `skills/locus/SKILL.md` and `references/language.md` changed during the run; their current content is not the tested snapshot. All trial skill copies match the frozen snapshot. A future rerun of the current skill therefore tests different content.

### Failures and visual findings

The missing Locus exports were sequence DSH runs 2/3, Pi run 2 and OpenCode run 3, plus flowchart DSH run 1 and OpenCode run 3. Five had no final `figure.locus`; OpenCode's final flowchart source failed E209 because it used an unsupported `end` property. The missing direct-SVG sequence was DSH run 3, which ended after planning without creating `figure.svg`. These were authoring/check failures, not timeouts.

- All Locus triangles passed geometry checks, but detached point labels and extended-line whitespace reduced readability in some images. Direct SVG sometimes omitted a visible incircle or placed the altitude/right-angle mark incorrectly.
- Successful Locus sequences often lacked a branch divider, used incorrect line styles, or showed visible routing-point clutter. Direct SVG's remaining defects included short activation bars and legends covered by participant headers.
- Locus's last DSH flowchart met the full visual requirements. Other Locus flowcharts sometimes missed the render highlight or used a tangential return arrow. Every direct-SVG flowchart missed the requested curved return loop, despite its higher overall score.

The debug CLI preserved **279 rejected-request archives**, representing **138 distinct failed source-input sets** after deduplicating original-file, stdin and candidate bodies within each trial. Archives include repeated native/coordinator checks and temporary contract sources; they are not distinct engine bugs. E209 appeared in 69 archives, E114 in 27 and E135 in 23. The saved index also includes assertion failures and non-fatal warning codes.

### Measurement limits and outputs

The run recorded **41,180,504 provider tokens**. Four trials have incomplete usage, covering four responses without usable usage data and four recorded provider failures.

This compares full workflows, including Locus's skill/assertion/check burden; it does not isolate language syntax or measure human usability. Three repetitions and one reviewer provide descriptive evidence, not statistical proof. Arrangement checks establish their documented conditions and do not replace visual review.

Reports, raw sources, images, ratings, provider requests, CLI rejection archives, frozen inputs, `rejected-request-summary.json` and `search-guard-observations.json` are retained locally under ignored `target/locus-skill-benchmark/run-20261008-3/`. The full comparison is `report.md`, with six per-task/workflow reports and `workflow-summary.json`. Offline verification passed 37 tests; maximum function cognitive complexity was 14 against a threshold of 15.

## Full rerun: 2026-10-08, protocol 1.1

The full benchmark now includes triangle, sequence, and flowchart tasks: **54 fresh trials**, covering three harnesses, two workflows, and three repetitions. The new flowchart tests a check-before-render workflow with a decision, yes/no branches, a curved repair loop, and a highlighted render step. See [README.md](README.md) for setup and gate definitions.

**These trials do not demonstrate reduced drawing effort with Locus.** Locus used more reported tokens in all nine task/harness pairs. Triangle visual quality improved slightly overall, while sequence and flowchart quality and export reliability were lower.

| Task | Workflow | Reported tokens | SVGs | Mean visual /100 | Locus/SVG token ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| triangle | locus | 3,963,215 | 9/9 | 90.3 | 2.50× |
| triangle | svg | 1,582,900 | 9/9 | 87.0 | — |
| sequence | locus | 16,634,399 | 8/9 | 69.2 | 15.29× |
| sequence | svg | 1,087,743 | 9/9 | 94.9 | — |
| flowchart | locus | 16,155,533 | 7/9 | 73.1 | 40.59× |
| flowchart | svg | 398,059 | 9/9 | 94.1 | — |

Locus exported 24/27 scored images, including five coordinator exports from checked sources when the native harness did not export. Direct SVG exported 27/27. Codex inspected all 51 exported images using the 40/25/20/15 rubric; ratings are bound to SVG hashes. Missing exports score zero. No failed trials were replaced.

The three missing Locus exports were sequence Pi run 3 and flowchart Pi run 2, which timed out without a final source, and flowchart OpenCode run 1, which exhausted its request budget and failed the independent required-title gate. Sequence Pi run 2 also timed out, but its checked source could be exported by the coordinator. Several other request-cap exits produced valid checked sources.

The same pinned harnesses and model settings were used: DeepSeek Flash, thinking disabled, temperature 0, 8,192 output tokens, 64 provider calls and 600 seconds per trial. Trial order interleaves workflows and rotates harness order. Frozen inputs and live benchmark input hashes were verified after the run.

### Captured failures and improvement candidates

The debug CLI retained **293 rejected-request archives**, representing **147 distinct failed inputs** after deduplicating original source, stdin, and candidate bodies within each trial. These counts include native commands, repeated checks, and coordinator validation, including temporary contract sources. They are not counts of distinct engine bugs. Provider request bodies are retained separately.

| Error code | Archives containing the code | Meaning |
| --- | ---: | --- |
| assert_failed | 96 | Authored or independent assertions failed |
| E403 | 45 | Relational solver did not converge |
| E114 | 43 | Multiple statements or trailing tokens on one line |
| E135 | 33 | Unknown or forward reference |
| E209 | 27 | Invalid point or geometry anchor |
| E404 | 18 | Degenerate seed or invalid placement/label option |
| E417 | 15 | Unsupported PNG raster dimensions |

One archive can contain multiple codes. The saved index includes every code, trial-level diagnostic messages, and example archive paths. Priorities suggested by this evidence are:

- Make the skill portable: its references point to a normative specification that is absent from isolated trial workspaces. Pi logs show broad filesystem searches in the sequence timeout trials and flowchart Pi run 2. This is a plausible contributor, not a proven cause of every timeout.
- Improve examples and feedback for one-statement-per-line syntax, reference order, and supported anchors. Inspect the preserved inputs before treating a solver rejection as an engine defect.
- Reduce exposed routing points and label leader clutter. Flowchart repair arrows sometimes arrive tangentially at a node, and branch labels can overlap visible anchor points.
- Strengthen sequence guidance and verification around branch dividers, dashed return styling, activation duration, and label placement. Arrangement checks alone did not catch these visual defects.
- Direct SVG also missed the requested curved repair loop in all nine flowcharts; some repair arrowheads had ambiguous attachment. Its higher total score does not mean every requirement was satisfied.

### Measurement limits and retention

The run recorded **39,821,849 provider tokens**. Seven trials have incomplete usage, covering nine responses without usable usage data; six provider failures were recorded. Token totals therefore represent reported usage.

This compares full workflows, including Locus's skill/assertion/check burden, and does not isolate language syntax or measure human usability. Three repetitions and one reviewer provide descriptive evidence, not statistical proof. Checks establish only their documented gate conditions; visual review remains necessary.

Generated reports, trial sources, images, ratings, provider requests, CLI rejection archives, frozen inputs, and `rejected-request-summary.json` are preserved locally under ignored `target/locus-skill-benchmark/run-20261008-flowchart-1/`. The full report is `report.md`, with per-task reports and `workflow-summary.json`. They are not committed benchmark inputs. Offline verification passed 32 tests; the benchmark code's maximum function cognitive complexity is 14 against a threshold of 15.

## Historical benchmark summary

**Trials:** 2026-10-07. **Summary:** 2026-10-08. This is the corrected historical experiment, preceding formal protocol 1.0. See [README.md](README.md) for reproducible setup, protocol and metric definitions.

## Finding

**These 36 trials do not demonstrate that Locus makes agent drawing easier than direct SVG.** Locus used more tokens in every task/harness pair. Triangle quality improved slightly overall; sequence quality and export reliability were worse. This compares full workflows, including Locus’s extra skill/assertion/check burden, and does not establish human usability.

DeepSeek Flash, thinking disabled, temperature 0, 8,192 output tokens, three fresh runs per task/workflow/harness, 64 provider calls and 600 seconds per trial. DSH 0.1.5-rc.2, Pi 0.84.4, OpenCode 1.18.35. Missing exports score zero.

## Overall results

| Task | Workflow | Tokens | SVGs | Mean visual /100 |
| --- | --- | ---: | ---: | ---: |
| triangle | locus | 6,269,817 | 9/9 | 90.2 |
| triangle | svg | 1,040,493 | 9/9 | 85.4 |
| sequence | locus | 14,524,958 | 6/9 | 51.7 |
| sequence | svg | 1,123,717 | 9/9 | 94.0 |

Locus used **6.03×** the triangle tokens for a **4.8-point** mean visual improvement. For sequences it used **12.93×** the tokens, with a **42.3-point** mean visual decrease.

## Harness results

Three trials per row. Revisions and scores appear in run order.

| Task | Harness | Workflow | Tokens | Revisions 1/2/3 | Scores 1/2/3 |
| --- | --- | --- | ---: | --- | --- |
| triangle | dsh | locus | 2,895,195 | 6, 805, 2 | 93, 66, 95 |
| triangle | dsh | svg | 603,964 | 3, 0, 2 | 89, 96, 94 |
| triangle | pi | locus | 1,850,204 | 11, 7, 14 | 90, 97, 93 |
| triangle | pi | svg | 221,043 | 1, 0, 1 | 78, 99, 99 |
| triangle | opencode | locus | 1,524,418 | 4, 3, 1634 | 95, 90, 93 |
| triangle | opencode | svg | 215,486 | 0, 1, 0 | 71, 72, 71 |
| sequence | dsh | locus | 6,719,502 | 4, 6, 18 | 77, 65, 75 |
| sequence | dsh | svg | 895,580 | 1, 0, 3 | 89, 99, 89 |
| sequence | pi | locus | 1,220,639 | 1, 0, 0 | 89, 0, 0 |
| sequence | pi | svg | 87,771 | 0, 0, 0 | 92, 93, 97 |
| sequence | opencode | locus | 6,584,817 | 8, 10, 7 | 0, 74, 85 |
| sequence | opencode | svg | 140,366 | 0, 0, 0 | 97, 90, 100 |

## Interpretation and failures

Codex inspected rendered images. The rubric is correctness 40, completeness 25, readability 20 and styling 15; ratings are bound to SVG hashes. Parsing and arrangement checks are not image correctness scores.

Locus’s geometry checks helped Pi/OpenCode triangles. Direct SVG defects included hidden incircles and incorrect right-angle marks. Corrected DSH Locus run 2 passed geometry checks but placed labels misleadingly and scored 66.

Locus sequences suffered missing/misplaced branch dividers, wrong dashed/solid styles, short activation bars and routing-point clutter. Pi run 2 ended with an unfinished plan at the output limit; Pi run 3 timed out during a broad filesystem search; OpenCode run 1 exhausted its call budget and failed a containment assertion. These three had no scored SVG. Four other call-cap exits still exported checked sources and received image ratings.

Revisions are distinct observed content versions after the first, not API retries. The Locus triangle total of 2,486 is dominated by bulk candidate searches of 805 and 1,634 revisions; SVG triangles had 8. Sequence totals were 54 for Locus and 4 for SVG. Locus checks and SVG boundary observations have different granularity, so these counts are not directly comparable effort measures.

The local prompt audit captured initial requests from all three harnesses without paid API calls. Main message text was 8,897 / 6,534 / 12,791 characters for DSH / Pi / OpenCode. None contained a complete eligible tracked project file; the repository source tree was not automatically appended. Skill reads, source and tool/check history grow later prompts and are counted again on subsequent calls. Old paid-trial logs retained usage/hashes rather than full request bodies.

## Exclusions

| Category | Tokens | Scored? |
| --- | ---: | --- |
| Corrected 36 trials | 22,958,985 | Yes |
| Four replaced benchmark-fault trials | 5,032,244 | No |
| Setup/pilot runs | 2,654,185 | No |
| Total recorded categories | 30,645,414 | — |

Three original DSH triangles and the first DSH sequence trial were replaced after benchmark logs attempted writes outside the native sandbox. Their usage is preserved separately. Genuine task failures remain scored. All scored trials had complete successful-response usage and zero recorded provider HTTP failures; this does not mean zero tool failures or local budget rejections.

## Limits and artifact retention

Only two tasks, three repetitions and one reviewer were used. Native tools/vision, batching and provider cache differ. Locus receives six assertions, fixed IDs and stronger automated feedback; SVG receives parsing and a preview renderer. The saved sequence prompt ambiguously called the inward self-call leg a “return arrow”; the current protocol clarifies solid styling without retroactively rerunning or changing these ratings. Formal protocol 1.0 also interleaves workflows and captures full request bodies; the historical runs used separate batches.

Generated sources, SVGs, metrics, ratings, prompt captures and the previous runner are preserved locally under ignored `target/locus-skill-benchmark/archive/20261008/`. They are not required to reproduce the formal benchmark and are not part of the repository inputs. The summary above is self-contained; a fresh checkout uses the protocol/fixtures/dependency lock described in the README.
