#!/usr/bin/env python3
"""Report measured runs with explicit, image-specific visual ratings."""

import argparse
import hashlib
import json
from pathlib import Path
from statistics import mean

LIMITS = json.loads(Path(__file__).with_name("provider_protocol.json").read_text())["rubric"]


def write_json(path, data):
    path = Path(path)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(data, indent=2) + "\n")
    temporary.replace(path)


def rating_total(rating, result, folder):
    if rating is None:
        return None
    svg = folder / f"{result['harness']}-{result['repetition']}" / "figure.svg"
    if not svg.is_file():
        if result.get("svg"):
            raise ValueError(f"Declared SVG is missing: {svg}")
        return 0
    if rating.get("svg_sha256") != hashlib.sha256(svg.read_bytes()).hexdigest():
        raise ValueError(f"Rating does not match current SVG: {svg}")
    scores = rating["scores"]
    if scores is None:
        return None
    if not rating.get("reviewer") or not rating.get("observations"):
        raise ValueError("Inspected images require reviewer identity and observations")
    if set(scores) != set(LIMITS):
        raise ValueError("Rating must include all four rubric components")
    for component, maximum in LIMITS.items():
        value = scores[component]
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= value <= maximum:
            raise ValueError(f"{component} score outside rubric range")
    return sum(scores.values())


def comparison_rows(results, ratings, folder):
    rows = []
    for harness in dict.fromkeys(row["harness"] for row in results):
        group = [row for row in results if row["harness"] == harness]
        scores = [rating_total(ratings.get(f"{harness}-{row['repetition']}"), row, folder) for row in group]
        score = "unrated" if any(value is None for value in scores) else f"{mean(scores):.1f}/100"
        retries = ", ".join(str(row["retry_count"]) for row in group)
        rows.append(f"| {harness} | {len(group)} | {sum(r['total_tokens'] for r in group):,} | "
                    f"{mean(r['total_tokens'] for r in group):,.0f} | {retries} | "
                    f"{sum(r['contract_passed'] for r in group)}/{len(group)} | {score} |")
    return rows


def report(folder):
    summary = json.loads((folder / "summary.json").read_text())
    path = folder / "ratings.json"
    ratings = json.loads(path.read_text()) if path.exists() else {}
    results = summary["results"]
    direct_svg = summary.get("backend") == "svg"
    title = (f"Direct SVG harness comparison: {summary.get('task')}" if direct_svg
             else f"Locus skill harness comparison: {summary.get('task', 'triangle')}")
    gate = "SVG parses" if direct_svg else "Checks pass"
    retry_definition = ("Revision counts are distinct SVG bytes observed at model-request boundaries "
                        "and final export, after the first. Multiple edits within a tool call may be missed."
                        if direct_svg else "Repair counts are observed checked source revisions after the first.")
    lines = [f"# {title}", "", f"Model: `{summary['model']}`. "
             f"Thinking disabled; temperature 0. Versions: {summary.get('harness_versions', {})}.", "",
             f"One drawing task; fresh sessions; {summary['runs_per_harness']} repetitions per harness.", "",
             f"| Harness | Runs | Total tokens | Mean tokens/run | Revisions by run | {gate} | Mean visual score |",
             "| --- | ---: | ---: | ---: | --- | ---: | ---: |"]
    lines.extend(comparison_rows(results, ratings, folder))
    lines.extend(["", retry_definition + " "
                  "API failures and repeated payloads are recorded separately. "
                  "See the benchmark README for definitions and measurement limits.", "",
                  "Visual scores are recorded reviewer assessments: geometry/flow correctness 40, completeness 25, "
                  "readability 20, and styling 15 points. Scores are subjective; this "
                  "small single-task sample is not a general harness ranking.", ""])
    if summary.get("execution_note"):
        lines.extend([summary["execution_note"], ""])
    if summary.get("setup_runs"):
        lines.extend(["Excluded setup/pilot runs exposed benchmark compatibility problems and are retained "
                      "separately in summary.json; they are not scored comparison trials.", ""])
    for observation in summary.get("observations", []):
        lines.extend([observation, ""])
    lines.extend(["## Direct SVG outputs", ""])
    for row in results:
        name = f"{row['harness']}-{row['repetition']}"
        rating = ratings.get(name)
        score = rating_total(rating, row, folder)
        lines.extend([f"### {name}", "", f"Tokens: {row['total_tokens']:,}; "
                      f"repairs: {row['retry_count']}; visual score: {score if score is not None else 'unrated'}.", ""])
        if (folder / name / "figure.svg").exists():
            lines.extend([f"![{name}]({folder / name / 'figure.svg'})", ""])
        if rating:
            lines.extend([rating["observations"], ""])
    (folder / "report.md").write_text("\n".join(lines))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    report(parser.parse_args().folder.resolve())
