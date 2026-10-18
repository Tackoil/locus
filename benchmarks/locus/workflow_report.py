#!/usr/bin/env python3
"""Compare Locus drawing effort with direct SVG inside each native harness."""

import json
from pathlib import Path
from statistics import mean

from report import rating_total, write_json


def load_groups(folder, task, backend):
    summary = json.loads((folder / "summary.json").read_text())
    ratings = json.loads((folder / "ratings.json").read_text())
    groups = []
    for harness in summary.get("harnesses", dict.fromkeys(row["harness"] for row in summary["results"])):
        rows = sorted((row for row in summary["results"] if row["harness"] == harness),
                      key=lambda row: row["repetition"])
        scores = [rating_total(ratings.get(f"{harness}-{row['repetition']}"), row, folder) for row in rows]
        if not rows or any(score is None for score in scores):
            raise ValueError(f"Inspect and rate every {task}/{backend}/{harness} image first")
        groups.append({"task": task, "backend": backend, "harness": harness, "runs": len(rows),
                       "usage_complete": all(row.get("usage_complete", False) for row in rows),
                       "total_tokens": sum(row["total_tokens"] for row in rows),
                       "provider_calls": sum(row["api_requests"] for row in rows),
                       "mean_elapsed_seconds": mean(row["elapsed_seconds"] for row in rows),
                       "revisions": [row["retry_count"] for row in rows], "visual_scores": scores,
                       "mean_visual_score": mean(scores),
                       "exported_svgs": sum(bool(row["svg"]) for row in rows),
                       "native_exports": sum(row["harness_exported_svg"] for row in rows),
                       "process_successes": sum(row["exit_code"] == 0 and not row["timed_out"] for row in rows),
                       "timed_out_runs": sum(row["timed_out"] for row in rows),
                       "checked_outputs": sum(row["contract_passed"] for row in rows),
                       "mean_correctness": mean(ratings[f"{harness}-{row['repetition']}"]["scores"]["geometry"] for row in rows),
                       "mean_completeness": mean(ratings[f"{harness}-{row['repetition']}"]["scores"]["completeness"] for row in rows),
                       "directory": str(folder)})
    return groups


def table_row(group):
    return (f"| {group['task']} | {group['harness']} | {group['backend']} | {group['total_tokens']:,} | "
            f"{group['provider_calls']} | {group['mean_elapsed_seconds']:.1f} | "
            f"{', '.join(map(str, group['revisions']))} | {group['exported_svgs']}/{group['runs']} | "
            f"{group['mean_visual_score']:.1f} |")


def paired_rows(groups):
    rows = []
    for task in dict.fromkeys(g["task"] for g in groups):
        for harness in dict.fromkeys(g["harness"] for g in groups if g["task"] == task):
            pair = {g["backend"]: g for g in groups if g["task"] == task and g["harness"] == harness}
            locus, svg = pair["locus"], pair["svg"]
            rows.append(f"| {task} | {harness} | {ratio(locus['total_tokens'], svg['total_tokens'])} | "
                        f"{locus['mean_correctness']:.1f} / {svg['mean_correctness']:.1f} | "
                        f"{locus['mean_completeness']:.1f} / {svg['mean_completeness']:.1f} | "
                        f"{locus['mean_visual_score'] - svg['mean_visual_score']:+.1f} | "
                        f"{'Yes' if criterion_met(locus, svg) else 'No'} |")
    return rows


def ratio(numerator, denominator):
    return f"{numerator / denominator:.2f}×" if denominator else "n/a (zero baseline tokens)"


def criterion_met(locus, svg):
    return (locus.get("usage_complete", True) and svg.get("usage_complete", True)
            and locus["exported_svgs"] == locus["runs"]
            and locus["total_tokens"] < svg["total_tokens"]
            and locus["mean_correctness"] >= svg["mean_correctness"]
            and locus["mean_completeness"] >= svg["mean_completeness"]
            and locus["mean_visual_score"] >= svg["mean_visual_score"])


def findings(groups):
    lines = ["## What this says about drawing effort", ""]
    for task in dict.fromkeys(g["task"] for g in groups):
        locus = [g for g in groups if g["task"] == task and g["backend"] == "locus"]
        svg = [g for g in groups if g["task"] == task and g["backend"] == "svg"]
        multiplier = ratio(sum(g["total_tokens"] for g in locus), sum(g["total_tokens"] for g in svg))
        locus_score = sum(g["mean_visual_score"] * g["runs"] for g in locus) / sum(g["runs"] for g in locus)
        svg_score = sum(g["mean_visual_score"] * g["runs"] for g in svg) / sum(g["runs"] for g in svg)
        lines.extend([f"For {task}, the Locus/SVG provider-token ratio was {multiplier}. "
                      f"Mean visual scores were {locus_score:.1f} for Locus and {svg_score:.1f} for SVG. "
                      f"Locus produced {sum(g['exported_svgs'] for g in locus)}/{sum(g['runs'] for g in locus)} "
                      f"exports; SVG produced {sum(g['exported_svgs'] for g in svg)}/{sum(g['runs'] for g in svg)}.", ""])
    tokens = {(g["task"], g["harness"], g["backend"]): g["total_tokens"] for g in groups}
    pairs = {(g["task"], g["harness"]) for g in groups}
    if all(tokens[task, harness, "locus"] > tokens[task, harness, "svg"] for task, harness in pairs):
        lines.extend(["Locus used more tokens in every task/harness pair. These trials do not "
                      "demonstrate reduced drawing effort with Locus.", ""])
    lines.extend(["Assess a quality improvement together with its additional effort. More reliable "
                  "geometry verification may be useful even when authoring takes more tokens, but that "
                  "does not establish that drawing became easier. Successful arrangement checks alone "
                  "cannot certify a correct diagram.", ""])
    return lines


def generate(args):
    inputs = args.inputs
    groups = [group for folder, task, backend in inputs for group in load_groups(folder.resolve(), task, backend)]
    groups.sort(key=lambda g: (g["task"], ("dsh", "pi", "opencode").index(g["harness"]), g["backend"]))
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = args.output / "manifest.json"
    experiment = json.loads(manifest.read_text()) if manifest.exists() else {}
    write_json(args.output / "workflow-summary.json", {"objective": "Whether Locus makes image drawing easier",
                                                       "protocol": experiment.get("protocol"),
                                                       "canonical": experiment.get("canonical"),
                                                       "groups": groups})
    lines = ["# Locus drawing effort benchmark", "",
             "Compare Locus with direct SVG **within each harness**, using the same image requirements. "
             "The target is lower effort to produce correct, complete, readable images. Harness rankings "
             "alone do not answer that question.", "",
             "DeepSeek Flash; thinking disabled; temperature 0; 8192 output-token cap. Trial counts and "
             "protocol settings are saved in manifest.json and each task summary. Failed trials remain "
             "in the denominators and token totals. No export scores 0. Available images require "
             "hash-bound reviewer ratings before this report can be generated.", "",
             "| Task | Harness | Workflow | Total tokens | Provider calls | Mean time/run (s) | Revisions 1/2/3 | SVGs | Mean visual /100 |",
             "| --- | --- | --- | ---: | ---: | ---: | --- | ---: | ---: |"]
    lines.extend(table_row(group) for group in groups)
    lines.extend(["", "## Locus versus SVG, paired by harness", "",
                  "A token multiplier above 1 means Locus used more tokens. Correctness is geometry or "
                  "diagram-flow correctness out of 40; completeness is out of 25. Visual score additionally "
                  "includes readability (20) and styling (15). Differences are Locus minus SVG.", "",
                  "The descriptive criterion requires all Locus exports, lower total tokens, "
                  "and no lower mean correctness, completeness or visual score than direct SVG. "
                  "Meeting it in a small sample is not statistical proof.", "",
                  "| Task | Harness | Token multiplier | Correctness: Locus / SVG | Completeness: Locus / SVG | Visual score difference | Criterion met |",
                  "| --- | --- | ---: | ---: | ---: | ---: | --- |"])
    lines.extend(paired_rows(groups))
    lines.extend([""] + findings(groups))
    lines.extend(["", "## Measurement limits", "",
                  "This is a comparison of the full authoring workflows. Validation and context follow "
                  "the experiment's saved protocol. Historical provider protocol 1.6 is exploratory: it exposed "
                  "filesystem/shell tools and has documented cross-workflow contamination. Provider protocol "
                  "locus-provider-http 2.0 uses restricted source-only tools, scoped Locus documentation, explicit "
                  "strict/layout flags, direct-SVG resource checks and separate author/tool/export timings. "
                  "Its evidence cannot be pooled with 1.6 or the separate Codex development protocol. "
                  "This workflow effort is included in the effort totals. The experiment does not isolate "
                  "language syntax from validation overhead and does not measure human usability.", "",
                  "Native validity does not verify requested geometry, diagram semantics, completeness or "
                  "readable layout. Those require image review. A check pass is not an image-quality pass. "
                  "Protocol 2.0 retains source/check provenance under raw/<run>/service/. Historical debug CLI "
                  "request archives remain in their original locations.", "",
                  "Protocol 2.0 counts distinct source candidates explicitly checked through the agent tool. "
                  "Historical SVG counts used file snapshots at model-request boundaries. These are lower bounds. Bulk candidate searches "
                  "can create hundreds of source revisions in one tool call; these are not API retries.", "",
                  "Provider calls include native background model requests and are measured by the API proxy. "
                  "Local rejections after the 64-request cap consume no provider tokens. Process exit 0 alone "
                  "does not establish completion. The coordinator exports a checked Locus source even if the "
                  "harness forgets to export in historical runs. Protocol 2.0 requires explicit final submission "
                  "and a passing strict layout check for Locus; legacy native-file-export counts do not measure "
                  "HTTP artifact requests.", "",
                  "Missing provider usage leaves token totals partial. The summary records usage_complete per "
                  "task/workflow/harness group; incomplete accounting cannot meet the efficiency criterion. "
                  "Costs are unmeasured; no currency estimate is inferred from token counts.", "",
                  "Trials ran in different batches. Native system prompts/tools, vision capability and provider "
                  "caching differ; cache cannot be reset. Small samples do not establish a general result.", "",
                  "## Saved outputs", ""])
    for folder, task, backend in inputs:
        lines.extend([f"[{task}: {backend}, all trials and image ratings]({folder.resolve() / 'report.md'})", ""])
    (args.output / "report.md").write_text("\n".join(lines))
