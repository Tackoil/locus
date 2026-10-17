#!/usr/bin/env python3
"""Measure complete, uncached CLI renders for every public example."""

import argparse
import hashlib
import json
import platform
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/locus")
    parser.add_argument("--output", type=Path, default=ROOT / "target/render-benchmark")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--limit", type=float, default=1.0)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--formats", nargs="+", choices=["svg", "png", "tikz"], default=["svg"])
    args = parser.parse_args()
    if min(args.runs, args.limit, args.timeout) <= 0:
        parser.error("runs, limit, and timeout must be positive")
    args.binary = args.binary.resolve()
    args.output = args.output.resolve()
    return args


def render(args, source, format_name, repetition):
    name = f"{source.parent.name}-{source.stem}-{repetition}"
    suffix = "tex" if format_name == "tikz" else format_name
    destination = args.output / f"{name}.{suffix}"
    destination.unlink(missing_ok=True)
    command = [str(args.binary), "render", str(source), "--no-dump", "-o", str(destination)]
    if format_name != "svg":
        command.append(f"--{format_name}")
    start = time.perf_counter()
    try:
        result = subprocess.run(command, capture_output=True, timeout=args.timeout, cwd=ROOT)
        seconds = time.perf_counter() - start
        successful = result.returncode == 0 and destination.is_file() and destination.stat().st_size > 0
        error = result.stderr.decode(errors="replace") if not successful else None
    except subprocess.TimeoutExpired:
        seconds, successful, error = time.perf_counter() - start, False, "render timed out"
    return {"source": str(source.relative_to(ROOT)), "format": format_name,
            "run": repetition, "seconds": seconds, "success": successful,
            "within_limit": successful and seconds <= args.limit, "error": error}


def main():
    args = arguments()
    args.output.mkdir(parents=True, exist_ok=True)
    # Load the executable once, excluding a newly built binary's OS verification.
    subprocess.run([str(args.binary), "--version"], check=True, capture_output=True)
    sources = sorted((ROOT / "examples/demonstrations").glob("*.locus"))
    if not sources:
        raise SystemExit("no examples found")
    report = {"platform": platform.platform(), "machine": platform.machine(),
              "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              "limit_seconds": args.limit, "source_count": len(sources), "results": []}
    for source in sources:
        for format_name in args.formats:
            for repetition in range(1, args.runs + 1):
                row = render(args, source, format_name, repetition)
                report["results"].append(row)
                print(f"{row['seconds']:.3f}s {format_name} {row['source']} "
                      f"{'PASS' if row['within_limit'] else 'FAIL'}", flush=True)
                (args.output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    raise SystemExit(0 if all(row["within_limit"] for row in report["results"]) else 1)


if __name__ == "__main__":
    main()
