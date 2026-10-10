#!/usr/bin/env python3
"""Set up, check, run and report the paired Locus/SVG benchmark."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
from types import SimpleNamespace

from experiment import (PROTOCOL, TOOLS, completed_trial, default_tool, initialize,
                        preflight, save_rating_template)
from harnesses import write_json
import run
import run_svg
from report import report
from workflow_report import generate


def setup(args):
    TOOLS.mkdir(parents=True, exist_ok=True)
    for name in ("package.json", "package-lock.json"):
        shutil.copyfile(run.BENCHMARK / name, TOOLS / name)
    subprocess.run(["npm", "ci", "--prefix", str(TOOLS), "--no-audit", "--no-fund"], check=True)
    print(f"Locked tools installed in {TOOLS}")


def run_matrix(args):
    commands, identity = preflight(args)
    key = run.credentials()
    args.output = args.output.resolve()
    manifest = initialize(args, identity)
    args.model = PROTOCOL["model"]
    args.skill = args.output / "inputs/skill"
    args.inputs = args.output / "inputs/benchmark"
    for trial in manifest["trials"]:
        args.task = trial["task"]
        folder = args.output / trial["backend"] / args.task
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / "summary.json"
        if path.exists():
            summary = json.loads(path.read_text())
        else:
            summary = (run.metadata(args, commands) if trial["backend"] == "locus"
                       else run_svg.metadata(args, args.task, commands))
            summary.update(protocol_id=PROTOCOL["id"], protocol_version=PROTOCOL["version"],
                           canonical=manifest["canonical"], harnesses=args.harnesses)
        result = completed_trial(folder, trial)
        if result is None:
            result = execute_trial(args, trial, folder, key, commands)
        if not any(r["harness"] == result["harness"] and r["repetition"] == result["repetition"]
                   for r in summary["results"]):
            summary["results"].append(result)
        write_json(path, summary)
        save_rating_template(folder, result)
    print(f"Trials complete. Inspect previews and fill ratings.json, then run: benchmark.py report --output {args.output}")


def execute_trial(args, trial, folder, key, commands):
    if trial["backend"] == "locus":
        return run.run_one(args, trial["harness"], trial["repetition"], folder, key, commands)
    return run_svg.run_one(args, trial["task"], trial["harness"], trial["repetition"], folder, key, commands)


def report_matrix(args):
    root = args.output.resolve()
    manifest = json.loads((root / "manifest.json").read_text())
    inputs = []
    for task in manifest["settings"]["tasks"]:
        for backend in PROTOCOL["backends"]:
            folder = root / backend / task
            summary = json.loads((folder / "summary.json").read_text())
            expected = [t for t in manifest["trials"] if t["task"] == task and t["backend"] == backend]
            actual = {(r["harness"], r["repetition"]) for r in summary["results"]}
            if actual != {(t["harness"], t["repetition"]) for t in expected}:
                raise ValueError(f"Incomplete experiment: {folder}")
            report(folder)
            inputs.append((folder, task, backend))
    generate(SimpleNamespace(inputs=inputs, output=root))
    print(root / "report.md")


def parser():
    cli = argparse.ArgumentParser(description=__doc__)
    subcommands = cli.add_subparsers(dest="action", required=True)
    subcommands.add_parser("setup", help="Install locked Node tools locally; no API calls")
    for action in ("check", "run"):
        command = subcommands.add_parser(action)
        for name in PROTOCOL["harnesses"]:
            command.add_argument(f"--{name}", default=default_tool(name))
        command.add_argument("--node", default="node")
        command.add_argument("--sharp", default=str(TOOLS / "node_modules/sharp"))
        command.add_argument("--locus", type=Path, default=run.REPOSITORY / "target/debug/locus")
        command.add_argument("--harnesses", nargs="+", choices=PROTOCOL["harnesses"], default=PROTOCOL["harnesses"])
        if action == "run":
            command.add_argument("--tasks", nargs="+", choices=PROTOCOL["tasks"], default=PROTOCOL["tasks"])
            for name in ("runs", "max_requests", "timeout"):
                command.add_argument("--" + name.replace("_", "-"), type=int, default=PROTOCOL[name])
            command.add_argument("--output", type=Path, required=True)
            command.add_argument("--resume", action="store_true")
    command = subcommands.add_parser("report", help="Generate a comparison from completed, rated trials")
    command.add_argument("--output", type=Path, required=True)
    return cli


def main():
    cli = parser()
    args = cli.parse_args()
    try:
        if args.action == "check":
            print(json.dumps(preflight(args)[1], indent=2))
        elif args.action == "setup":
            setup(args)
        elif args.action == "run":
            if min(args.runs, args.max_requests, args.timeout) < 1:
                raise ValueError("Runs, request budget and timeout must be positive")
            if len(set(args.tasks)) != len(args.tasks) or len(set(args.harnesses)) != len(args.harnesses):
                raise ValueError("Tasks and harnesses must not contain duplicates")
            run_matrix(args)
        else:
            report_matrix(args)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        cli.exit(1, f"Benchmark: {error}\n")


if __name__ == "__main__":
    main()
