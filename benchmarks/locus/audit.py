#!/usr/bin/env python3
"""Read-only workflow/provenance audit, including exploratory pre-HTTP transcripts."""
import argparse
import json
from pathlib import Path
import re
import shlex


def locus_command(arguments):
    try:
        values = json.loads(arguments)
        command = values.get("command", "")
    except (ValueError, TypeError, AttributeError):
        return False
    # Whitespace splitting preserves a quoted diagram label as one token.
    lexer = shlex.shlex(command, posix=True, punctuation_chars=";&|()")
    lexer.whitespace_split = True
    tokens = list(lexer)
    return any(Path(token).name == "locus" and tokens[index + 1] in
               {"check", "render", "describe", "inspect"}
               for index, token in enumerate(tokens[:-1]))


def call_flags(name, arguments, workflow):
    flags = set()
    lower = arguments.lower()
    if re.search(r"(?:\.\./|\bls\s+\.\.|\bpwd\b)", lower):
        flags.add("parent_discovery_or_reference")
    if re.search(r"(?:inputs/|ratings\.json|report\.md|metrics\.json|\.env\b|credentials)", lower):
        flags.add("protected_input_or_evidence_reference")
    if workflow == "svg":
        if re.search(r"(?:inputs/skill/|locus-skill/|skills/locus/|references/language\.md)", lower):
            flags.add("cross_workflow_skill_access")
        if locus_command(arguments) or re.search(r"locus 0 beta [0-9]", lower):
            flags.add("wrong_workflow_producer_tool")
    return flags


def transcript_flags(payload, workflow, restricted=False):
    flags = []
    expected = {"drawing", "scoped_drawing"}
    for message in payload.get("messages", []):
        for call in message.get("tool_calls", []):
            function = call.get("function", {})
            name, arguments = function.get("name", ""), function.get("arguments", "")
            found = call_flags(name, arguments, workflow)
            if restricted and name not in expected:
                found.add("forbidden_tool_attempt")
            if found:
                flags.append({"call_id": call.get("id"), "tool": name, "flags": sorted(found)})
    if restricted:
        names = {tool.get("function", {}).get("name") for tool in payload.get("tools", [])}
        if not names or not names <= expected or len(names) != 1:
            flags.append({"flags": ["forbidden_or_missing_tool_catalog"], "tools": sorted(str(n) for n in names)})
    return flags


def audit_trial(folder, workflow, restricted=False):
    findings = []
    for path in sorted((folder / "requests").glob("*.json")):
        for flag in transcript_flags(json.loads(path.read_text()), workflow, restricted):
            findings.append({"request": path.name, **flag})
    operations = folder / "service/operations.jsonl"
    if operations.exists():
        for row in map(json.loads, operations.read_text().splitlines()):
            if row.get("producer") != workflow:
                findings.append({"flags": ["wrong_workflow_producer_tool"], "operation": row.get("operation")})
    return {"folder": str(folder), "workflow": workflow, "findings": findings,
            "interpretation": "References/attempts are flagged; these alone do not prove successful file reads"}


def audit(root):
    manifest = json.loads((root / "manifest.json").read_text())
    restricted = manifest["protocol"]["version"] == "2.0"
    return {"audit_version": "2.0", "protocol_version": manifest["protocol"]["version"],
            "trials": [audit_trial(root / t["backend"] / t["task"] / "raw" /
                                   f"{t['harness']}-{t['repetition']}", t["backend"], restricted)
                       for t in manifest["trials"]]}


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("root", type=Path)
    print(json.dumps(audit(cli.parse_args().root), indent=2))
