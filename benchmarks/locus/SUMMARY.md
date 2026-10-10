# Historical benchmark summary

**Trials:** 2026-10-07. **Summary:** 2026-10-08. This is the corrected historical experiment, preceding formal protocol 1.0. See [README.md](README.md) for reproducible setup, protocol and metric definitions.

## Finding

**These 36 trials do not demonstrate that Locus makes agent drawing easier than direct SVG.** Locus used more tokens in every task/harness pair. Triangle quality improved slightly overall; sequence quality and export reliability were worse. This compares full workflows, including Locus’s extra skill/assertion/check burden, and does not establish human usability.

DeepSeek Flash, thinking disabled, temperature 0, 8,192 output tokens, three fresh runs per task/workflow/harness, 64 provider calls and 600 seconds per trial. DSH 0.1.5-rc.2, Pi 0.84.4, OpenCode 1.18.35. Missing exports score zero.

## Overall results

| Task | Workflow | Tokens | Estimated CNY | SVGs | Mean visual /100 |
| --- | --- | ---: | ---: | ---: | ---: |
| triangle | locus | 6,269,817 | ¥0.613562 | 9/9 | 90.2 |
| triangle | svg | 1,040,493 | ¥0.269358 | 9/9 | 85.4 |
| sequence | locus | 14,524,958 | ¥1.314446 | 6/9 | 51.7 |
| sequence | svg | 1,123,717 | ¥0.268753 | 9/9 | 94.0 |

Locus used **6.03×** the triangle tokens and **2.28×** the estimated cost for a **4.8-point** mean visual improvement. For sequences it used **12.93×** the tokens and **4.89×** the cost, with a **42.3-point** mean visual decrease.

## Harness results

Three trials per row. Revisions and scores appear in run order.

| Task | Harness | Workflow | Tokens | Estimated CNY | Revisions 1/2/3 | Scores 1/2/3 |
| --- | --- | --- | ---: | ---: | --- | --- |
| triangle | dsh | locus | 2,895,195 | ¥0.256457 | 6, 805, 2 | 93, 66, 95 |
| triangle | dsh | svg | 603,964 | ¥0.127044 | 3, 0, 2 | 89, 96, 94 |
| triangle | pi | locus | 1,850,204 | ¥0.194237 | 11, 7, 14 | 90, 97, 93 |
| triangle | pi | svg | 221,043 | ¥0.082982 | 1, 0, 1 | 78, 99, 99 |
| triangle | opencode | locus | 1,524,418 | ¥0.162868 | 4, 3, 1634 | 95, 90, 93 |
| triangle | opencode | svg | 215,486 | ¥0.059332 | 0, 1, 0 | 71, 72, 71 |
| sequence | dsh | locus | 6,719,502 | ¥0.535288 | 4, 6, 18 | 77, 65, 75 |
| sequence | dsh | svg | 895,580 | ¥0.163555 | 1, 0, 3 | 89, 99, 89 |
| sequence | pi | locus | 1,220,639 | ¥0.237120 | 1, 0, 0 | 89, 0, 0 |
| sequence | pi | svg | 87,771 | ¥0.050263 | 0, 0, 0 | 92, 93, 97 |
| sequence | opencode | locus | 6,584,817 | ¥0.542038 | 8, 10, 7 | 0, 74, 85 |
| sequence | opencode | svg | 140,366 | ¥0.054935 | 0, 0, 0 | 97, 90, 100 |

## Interpretation and failures

Codex inspected rendered images. The rubric is correctness 40, completeness 25, readability 20 and styling 15; ratings are bound to SVG hashes. Parsing and arrangement checks are not image correctness scores.

Locus’s geometry checks helped Pi/OpenCode triangles. Direct SVG defects included hidden incircles and incorrect right-angle marks. Corrected DSH Locus run 2 passed geometry checks but placed labels misleadingly and scored 66.

Locus sequences suffered missing/misplaced branch dividers, wrong dashed/solid styles, short activation bars and routing-point clutter. Pi run 2 ended with an unfinished plan at the output limit; Pi run 3 timed out during a broad filesystem search; OpenCode run 1 exhausted its call budget and failed a containment assertion. These three had no scored SVG. Four other call-cap exits still exported checked sources and received image ratings.

Revisions are distinct observed content versions after the first, not API retries. The Locus triangle total of 2,486 is dominated by bulk candidate searches of 805 and 1,634 revisions; SVG triangles had 8. Sequence totals were 54 for Locus and 4 for SVG. Locus checks and SVG boundary observations have different granularity, so these counts are not directly comparable effort measures.

The local prompt audit captured initial requests from all three harnesses without paid API calls. Main message text was 8,897 / 6,534 / 12,791 characters for DSH / Pi / OpenCode. None contained a complete eligible tracked project file; the repository source tree was not automatically appended. Skill reads, source and tool/check history grow later prompts and are counted again on subsequent calls. Old paid-trial logs retained usage/hashes rather than full request bodies.

## Exclusions and cost

| Category | Tokens | Estimated CNY | Scored? |
| --- | ---: | ---: | --- |
| Corrected 36 trials | 22,958,985 | ¥2.466119 | Yes |
| Four replaced benchmark-fault trials | 5,032,244 | ¥0.412603 | No |
| Setup/pilot runs | 2,654,185 | ¥0.269723 | No |
| Total recorded categories | 30,645,414 | ¥3.148445 | — |

Three original DSH triangles and the first DSH sequence trial were replaced after benchmark logs attempted writes outside the native sandbox. Their usage is preserved separately. Genuine task failures remain scored. Costs use provider usage and historical 2026-10-07 CNY rates, not invoice/current prices. All scored trials had complete successful-response usage and zero recorded provider HTTP failures; this does not mean zero tool failures or local budget rejections.

## Limits and artifact retention

Only two tasks, three repetitions and one reviewer were used. Native tools/vision, batching and provider cache differ. Locus receives six assertions, fixed IDs and stronger automated feedback; SVG receives parsing and a preview renderer. The saved sequence prompt ambiguously called the inward self-call leg a “return arrow”; the current protocol clarifies solid styling without retroactively rerunning or changing these ratings. Formal protocol 1.0 also interleaves workflows and captures full request bodies; the historical runs used separate batches.

Generated sources, SVGs, metrics, ratings, prompt captures and the previous runner are preserved locally under ignored `target/locus-skill-benchmark/archive/20261008/`. They are not required to reproduce the formal benchmark and are not part of the repository inputs. The summary above is self-contained; a fresh checkout uses the protocol/fixtures/dependency lock described in the README.
