#!/usr/bin/env python3
"""Prepare isolated drawing trials and record evidence without making model calls."""

import argparse
import hashlib
import json
import math
import os
import shlex
import shutil
import subprocess
import sys
from pathlib import Path
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n")


def read_json(path):
    return json.loads(path.read_text())


def output_text(value):
    return value.decode(errors="replace") if isinstance(value, bytes) else (value or "")


def command(argv, cwd=None, env=None):
    try:
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=60)
        return {"argv": list(map(str, argv)), "exit_code": result.returncode,
                "stdout": result.stdout, "stderr": result.stderr, "timed_out": False}
    except subprocess.TimeoutExpired as error:
        return {"argv": list(map(str, argv)), "exit_code": None,
                "stdout": output_text(error.stdout), "stderr": output_text(error.stderr), "timed_out": True}


def identity(path):
    resolved = Path(path).expanduser().resolve(strict=True)
    return {"path": str(resolved), "sha256": digest(resolved)}


def balanced_trials(protocol, repetitions, agent):
    trials = []
    for repetition in range(1, repetitions + 1):
        for index, task in enumerate(protocol["tasks"]):
            workflows = protocol["workflows"][:]
            if (index + repetition) % 2 == 0:
                workflows.reverse()
            for workflow in workflows:
                trials.append({"id": f"{agent}-{task}-{workflow}-{repetition}",
                               "task": task, "workflow": workflow,
                               "repetition": repetition, "agent": agent})
    return trials


def freeze_inputs(output):
    inputs = output / "inputs"
    inputs.mkdir()
    for name in ("tasks", "templates"):
        shutil.copytree(HERE / name, inputs / name)
    shutil.copytree(ROOT / "skills" / "locus", inputs / "skill")
    for name in ("protocol.json", "benchmark.py", "render_svg.js"):
        shutil.copy2(HERE / name, inputs / name)
    return {str(path.relative_to(output)): digest(path)
            for path in sorted(inputs.rglob("*")) if path.is_file()}


def workflow_instruction(trial, work, locus):
    if trial["workflow"] == "svg":
        return "Author native SVG XML directly using SVG coordinates and elements. Do not use Locus or any diagram DSL, source generator, or prior solution."
    return (f"Use Locus 0.0.0-beta.6. First read {work / 'skill' / 'SKILL.md'} and its language reference. "
            "Author figure.locus with relational geometry, without coordinates or pins. "
            f"Run {locus} check figure.locus --json, then "
            f"{locus} render figure.locus --background white -o figure.svg. "
            "Use the installed CLI and the frozen skill as your language documentation.")


def prepare_trial(output, trial, args):
    folder = output / "trials" / trial["id"]
    work = folder / "work"
    work.mkdir(parents=True)
    if trial["workflow"] == "locus":
        shutil.copytree(output / "inputs" / "skill", work / "skill")
    template = (output / "inputs/templates/common.txt").read_text()
    prompt = template.format(workspace=work, sharp=shlex.quote(args.sharp), node=shlex.quote(args.node),
                             renderer=output / "inputs/render_svg.js",
                             workflow_instruction=workflow_instruction(trial, work, args.locus),
                             task_prompt=(output / "inputs/tasks" / f"{trial['task']}.txt").read_text())
    (folder / "prompt.txt").write_text(prompt)
    trial["prompt_sha256"] = digest(folder / "prompt.txt")


def prepare(args):
    output = Path(args.output).resolve()
    if output.exists():
        raise ValueError("Output already exists; choose a fresh experiment directory")
    if args.repetitions < 1 or not args.agent.replace("-", "").isalnum():
        raise ValueError("Use positive repetitions and an alphanumeric agent name")
    args.locus = str(Path(args.locus).resolve(strict=True))
    args.node = str(Path(shutil.which(args.node) or args.node).resolve(strict=True))
    args.sharp = str(Path(args.sharp).resolve(strict=True))
    version = command([args.locus, "--version"])
    protocol = read_json(HERE / "protocol.json")
    if version["exit_code"] or protocol["locus_version"] not in version["stdout"]:
        raise ValueError("Locus executable must report 0.0.0-beta.6")
    output.mkdir(parents=True)
    hashes = freeze_inputs(output)
    trials = balanced_trials(protocol, args.repetitions, args.agent)
    for trial in trials:
        prepare_trial(output, trial, args)
    manifest = {"protocol": protocol, "trials": trials, "input_hashes": hashes,
                "locus": identity(args.locus), "locus_version": version,
                "git": command(["git", "rev-parse", "HEAD"], ROOT),
                "git_status": command(["git", "status", "--short"], ROOT),
                "python": sys.version, "tokens": None, "cost": None,
                "execution": "Independent fresh agent sessions; externally dispatched, then recorded",
                "renderer": {"node": args.node, "sharp": args.sharp,
                             "node_identity": identity(args.node),
                             "sharp_package": identity(Path(args.sharp) / "package.json"),
                             "node_version": command([args.node, "--version"])},
                "canonical": False}
    write_json(output / "manifest.json", manifest)
    print(f"Prepared {len(trials)} trials in {output}")


def load_experiment(output):
    manifest = read_json(output / "manifest.json")
    for relative, expected in manifest["input_hashes"].items():
        if digest(output / relative) != expected:
            raise ValueError(f"Frozen input changed: {relative}")
    if identity(manifest["locus"]["path"]) != manifest["locus"]:
        raise ValueError("Locus executable changed after preparation")
    for tool in ("node_identity", "sharp_package"):
        recorded = manifest.get("renderer", {}).get(tool)
        if recorded and identity(recorded["path"]) != recorded:
            raise ValueError(f"Preview runtime changed: {tool}")
    return manifest


def validate_work_skill(manifest, work):
    expected = {name.removeprefix("inputs/skill/"): checksum
                for name, checksum in manifest["input_hashes"].items() if name.startswith("inputs/skill/")}
    actual = {str(path.relative_to(work / "skill")): digest(path)
              for path in (work / "skill").rglob("*") if path.is_file()}
    if expected != actual:
        raise ValueError("Trial skill differs from frozen skill")


def check_svg(path):
    try:
        source = path.read_bytes()
        if b"<!DOCTYPE" in source.upper() or b"<!ENTITY" in source.upper():
            raise ValueError("DTD and entity declarations are unsupported")
        root = ET.fromstring(source)
        if root.tag != "{http://www.w3.org/2000/svg}svg":
            raise ValueError("Expected SVG root with the SVG namespace")
        return {"passed": True, "scope": "XML syntax and SVG root only"}
    except (OSError, ET.ParseError, ValueError) as error:
        return {"passed": False, "error": str(error)}


def validate_locus(manifest, work, folder):
    locus = manifest["locus"]["path"]
    check = command([locus, "check", "figure.locus", "--json"], work)
    render = command([locus, "render", "figure.locus", "--background", "white",
                      "-o", str(folder / "verified.svg")], work)
    write_json(folder / "native-check.json", check)
    write_json(folder / "native-render.json", render)
    authored = work / "figure.svg"
    verified = folder / "verified.svg"
    match = authored.is_file() and verified.is_file() and digest(authored) == digest(verified)
    return {"passed": check["exit_code"] == 0 and render["exit_code"] == 0 and match,
            "check_exit_code": check["exit_code"], "render_exit_code": render["exit_code"],
            "svg_matches_source": match, "scope": "Native solve/check and deterministic SVG source correspondence"}


def record(args):
    output = Path(args.output).resolve()
    manifest = load_experiment(output)
    trial = next((t for t in manifest["trials"] if t["id"] == args.trial), None)
    if trial is None:
        raise ValueError("Unknown trial")
    folder = output / "trials" / args.trial
    work = folder / "work"
    if (folder / "metrics.json").exists():
        raise ValueError("Trial already recorded; do not overwrite retained evidence")
    if digest(folder / "prompt.txt") != trial["prompt_sha256"]:
        raise ValueError("Trial prompt changed")
    if trial["workflow"] == "locus":
        validate_work_skill(manifest, work)
    if args.elapsed_seconds is not None and (not math.isfinite(args.elapsed_seconds) or args.elapsed_seconds < 0):
        raise ValueError("Elapsed seconds must be finite and nonnegative")
    svg = work / "figure.svg"
    validation = check_svg(svg)
    native = validate_locus(manifest, work, folder) if trial["workflow"] == "locus" else validation
    metrics = {"trial": args.trial, "status": args.status, "elapsed_seconds": args.elapsed_seconds,
               "tokens": None, "cost": None, "provider_calls": None, "revisions": None,
               "svg": validation, "native": native, "svg_sha256": digest(svg) if svg.is_file() else None,
               "source_sha256": digest(work / "figure.locus") if (work / "figure.locus").is_file() else None,
               "source_bytes": (work / "figure.locus").stat().st_size if (work / "figure.locus").is_file() else None,
               "svg_bytes": svg.stat().st_size if svg.is_file() else None,
               "elapsed_scope": "Coordinator supplied wall time; includes only the interval actually timed"}
    write_json(folder / "metrics.json", metrics)
    print(json.dumps(metrics, indent=2))


def completed_metrics(output, manifest):
    entries = []
    for trial in manifest["trials"]:
        folder = output / "trials" / trial["id"]
        metrics = read_json(folder / "metrics.json")
        if digest(folder / "prompt.txt") != trial["prompt_sha256"]:
            raise ValueError(f"Trial prompt changed: {trial['id']}")
        if trial["workflow"] == "locus":
            validate_work_skill(manifest, folder / "work")
            source = folder / "work/figure.locus"
            actual_source = digest(source) if source.is_file() else None
            if actual_source != metrics["source_sha256"]:
                raise ValueError(f"Locus source changed after recording: {trial['id']}")
        svg = folder / "work/figure.svg"
        actual = digest(svg) if svg.is_file() else None
        if actual != metrics["svg_sha256"]:
            raise ValueError(f"SVG changed after recording: {trial['id']}")
        entries.append((trial, metrics, svg))
    return entries


def review(args):
    output = Path(args.output).resolve()
    manifest = load_experiment(output)
    entries = completed_metrics(output, manifest)
    folder = output / "review"
    folder.mkdir()
    key = {}
    ratings = {}
    # Hash sorting scrambles workflow/task order deterministically, without altering images.
    entries.sort(key=lambda item: hashlib.sha256(item[0]["id"].encode()).hexdigest())
    for index, (trial, metrics, svg) in enumerate(entries, 1):
        if metrics["svg_sha256"] is None:
            continue
        label = f"image-{index:03}"
        shutil.copy2(svg, folder / f"{label}.svg")
        shutil.copy2(output / "inputs/tasks" / f"{trial['task']}.txt", folder / f"{label}-task.txt")
        renderer = manifest["renderer"]
        environment = {**os.environ, "SVG_BENCH_SHARP": renderer["sharp"]}
        raster = command([renderer["node"], str(output / "inputs/render_svg.js"),
                          str(folder / f"{label}.svg"), str(folder / f"{label}.png")], env=environment)
        write_json(folder / f"{label}-render.json", raster)
        key[label] = trial["id"]
        ratings[label] = {"svg_sha256": metrics["svg_sha256"], "reviewer": "", "observations": "",
                          "scores": {component: None for component in manifest["protocol"]["rubric"]}}
    write_json(output / "review-key.json", key)
    write_json(folder / "ratings.json", ratings)
    print(f"Review only {folder}; keep review-key.json hidden from reviewer")


def rating_score(rating, metrics, rubric):
    if not rating:
        return None
    if rating["svg_sha256"] != metrics["svg_sha256"]:
        raise ValueError("Stale rating: SVG hash differs")
    if not rating.get("reviewer") or not rating.get("observations"):
        return None
    scores = rating["scores"]
    if set(scores) != set(rubric):
        raise ValueError("Rating components differ from rubric")
    for name, maximum in rubric.items():
        score = scores[name]
        if isinstance(score, bool) or not isinstance(score, (int, float)):
            raise ValueError(f"Invalid score for {name}")
        if not math.isfinite(score) or not 0 <= score <= maximum:
            raise ValueError(f"Score out of range for {name}")
    return sum(scores.values())


def report(args):
    output = Path(args.output).resolve()
    manifest = load_experiment(output)
    entries = completed_metrics(output, manifest)
    key_path = output / "review-key.json"
    key = read_json(key_path) if key_path.exists() else {}
    ratings_path = output / "review/ratings.json"
    ratings = read_json(ratings_path) if ratings_path.exists() else {}
    by_trial = {trial: ratings.get(label) for label, trial in key.items()}
    lines = ["# Locus beta 6 / native SVG benchmark", "",
             "Protocol 2.0; independent agent sessions. Development run, not statistical proof.", "",
             "| Trial | Status | Native check | SVG XML | Wall seconds | Visual /100 |",
             "| --- | --- | --- | --- | --- | --- |"]
    for trial, metrics, _ in entries:
        score = rating_score(by_trial.get(trial["id"]), metrics, manifest["protocol"]["rubric"])
        if metrics["svg_sha256"] is None:
            score = 0
        if args.require_reviewed and score is None:
            raise ValueError(f"Unreviewed image: {trial['id']}")
        lines.append(f"| {trial['id']} | {metrics['status']} | {metrics['native']['passed']} | "
                     f"{metrics['svg']['passed']} | {metrics['elapsed_seconds']} | {score if score is not None else 'unreviewed'} |")
    lines += ["", "Provider tokens, costs, request counts and revision counts are unavailable (null).",
              "No token-efficiency or cost claim can be made. Wall time includes orchestration effects.",
              "Native checks establish syntax/solve status only. Scores require actual image review;",
              "missing exports score zero. Review is source-blinded, though rendering style can reveal workflow."]
    (output / "report.md").write_text("\n".join(lines) + "\n")
    print(output / "report.md")


def parser():
    main = argparse.ArgumentParser(description=__doc__)
    commands = main.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--output", required=True)
    prepare_parser.add_argument("--locus", default=str(ROOT / "target/release/locus"))
    prepare_parser.add_argument("--agent", default="codex")
    prepare_parser.add_argument("--repetitions", type=int, default=1)
    prepare_parser.add_argument("--node", default="node")
    prepare_parser.add_argument("--sharp", default=str(HERE / "node_modules/sharp"))
    record_parser = commands.add_parser("record")
    record_parser.add_argument("--output", required=True)
    record_parser.add_argument("--trial", required=True)
    record_parser.add_argument("--status", choices=("completed", "failed", "timeout"), default="completed")
    record_parser.add_argument("--elapsed-seconds", type=float)
    for name in ("review", "report"):
        sub = commands.add_parser(name)
        sub.add_argument("--output", required=True)
        if name == "report":
            sub.add_argument("--require-reviewed", action="store_true")
    return main


def main():
    args = parser().parse_args()
    try:
        {"prepare": prepare, "record": record, "review": review, "report": report}[args.command](args)
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        sys.exit(f"benchmark: {error}")


if __name__ == "__main__":
    main()
