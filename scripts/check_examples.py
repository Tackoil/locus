#!/usr/bin/env python3
"""Audit draft fixtures without claiming to execute their geometry or layout.

Checks declaration forms, constructor arities, names/reference order, relation
modes, balanced delimiters, expansion counts, and stress-manifest membership.
This is not a full parser, dimensional type checker, solver, or renderer.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import ChainMap
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


ROOT = Path(__file__).resolve().parents[1]
KINDS = "point num line ray segment circle arc ellipse bspline polygon rect rounded_rect diamond label arrow tick right_angle text_block".split()
IDENT = r"[A-Za-z][A-Za-z0-9_']*"
REF = re.compile(IDENT + r"(?:\." + IDENT + r")*")
PROTECTED = re.compile(r'"(?:\\.|[^"\\])*"|\$(?:\\.|[^$\\])*\$|//[^\n]*|#[0-9A-Fa-f]{6}(?:[0-9A-Fa-f]{2})?\b')
CTOR = re.compile(r"^(" + "|".join(KINDS) + r")\s+(" + IDENT + r")\s*\(")
VALUE = re.compile(r"^(" + "|".join(KINDS) + r")\s+(" + IDENT + r")\s*=\s*")
CALL = re.compile(r"^(" + IDENT + r")\s+(" + IDENT + r")\s*\(")
GROUP = re.compile(r"^group\s+(" + IDENT + r")\s*\(")
FUNCTION = re.compile(r"^fn\s+(" + IDENT + r")\s*\(([^)]*)\)\s*\{$")
ARITIES = {
    "point": {0}, "line": {2}, "ray": {2}, "segment": {2},
    "circle": {1, 2}, "arc": {4}, "ellipse": {2, 3, 4},
    "rect": {0, 2, 3}, "diamond": {0, 2, 3},
    "rounded_rect": {0, 3, 4}, "label": {1, 2}, "arrow": {2},
    "tick": {2}, "right_angle": {3}, "text_block": {1},
}
BUILTINS = set(KINDS) | set("seg midpoint lerp intersect foot project rotate reflect translate on_circle extend centroid circumcenter incenter orthocenter center perpendicular parallel perp_bisector angle_bisector tangent_line circle_through circle_diameter circumcircle incircle angle_arc edge dist hdist vdist angle dir_angle ratio radius length area perimeter sqrt abs min max style_metric dash".split())
PREDICATES = set("center align_center_x align_center_y align_left align_right align_top align_bottom gap_x gap_y row column inset_left inset_right inset_top inset_bottom cover_x cover_y contain inside on avoid trim_endpoints tangent internally_tangent parallel perpendicular collinear concyclic midpoint between".split())
SYMBOLS = set("free where true false and or not tol deg rad textbook solid dashed dotted thin normal bold ink gray blue orange teal green red purple none tint auto N NE E SE S SW W NW round miter bevel butt square direct curved orthogonal left right above below start middle end title dot cross".split())
PROP_KEYS = set("stroke weight accent fill opacity cap join dir gap role near side route head bend n size place anchor mark padding".split())
CLASSES = {name: "class" for name in ("main", "auxiliary", "construction", "highlight")}
CLOSING = {")": "(", "]": "[", "}": "{"}


def mask(source: str) -> str:
    return PROTECTED.sub(lambda match: re.sub(r"[^\n]", " ", match.group()), source)


def delimiters(code: str) -> dict[int, int]:
    stack: list[tuple[int, str]] = []
    pairs: dict[int, int] = {}
    for index, char in enumerate(code):
        if char in "([{":
            stack.append((index, char))
        elif char in ")]}":
            if not stack or CLOSING[char] != stack[-1][1]:
                raise ValueError("unbalanced or mismatched delimiters")
            opening, _ = stack.pop()
            pairs[opening] = index
    if stack:
        raise ValueError("unclosed delimiter")
    return pairs


def arguments(source: str, opening: int) -> tuple[list[str], int]:
    code = mask(source)
    pairs = delimiters(code)
    ending = pairs[opening]
    slices: list[str] = []
    start = cursor = opening + 1
    while cursor < ending:
        if code[cursor] in "([{":
            cursor = pairs[cursor]
        elif code[cursor] == ",":
            slices.append(source[start:cursor].strip())
            start = cursor + 1
        cursor += 1
    final = source[start:ending].strip()
    if final or slices:
        slices.append(final)
    if any(not item for item in slices):
        raise ValueError("empty argument")
    return slices, ending


def references(source: str, scope: ChainMap[str, str]) -> None:
    code = mask(source)
    for match in REF.finditer(code):
        name = match.group().split(".")[0]
        if match.start() and code[match.start() - 1].isalnum():
            continue
        if name not in scope and name not in BUILTINS | PREDICATES | SYMBOLS | PROP_KEYS:
            raise ValueError(f"unknown or forward reference: {name}")


@dataclass
class Counts:
    objects: int = 0
    assertions: int = 0

    def add(self, other: Counts) -> None:
        self.objects += other.objects
        self.assertions += other.assertions


@dataclass
class Definition:
    params: list[str]
    counts: Counts = field(default_factory=Counts)


def declare(scope: ChainMap[str, str], name: str, kind: str) -> None:
    if name in scope.maps[0]:
        raise ValueError(f"duplicate name: {name}")
    scope[name] = kind


def solvable(code: str) -> None:
    if re.search(r"\b(?:or|not|inside|between)\b|[<>]", code):
        raise ValueError("assert-only condition in rel/where")


def where_mode(source: str) -> None:
    code = mask(source)
    found = re.search(r"\bwhere\s*\{", code)
    if found is None:
        return
    opening = found.end() - 1
    ending = delimiters(code)[opening]
    condition = code[opening + 1:ending]
    solvable(condition)
    if re.search(r"\btrim_endpoints\s*\(", condition):
        raise ValueError("clipping is a standalone operation, not a where condition")


def constructor(source: str, match: re.Match[str], scope: ChainMap[str, str], counts: Counts) -> None:
    kind, name = match.groups()
    args, ending = arguments(source, match.end() - 1)
    valid = len(args) in ARITIES.get(kind, set())
    if kind == "polygon":
        valid = len(args) >= 3
    elif kind == "bspline":
        valid = len(args) >= 4
    if not valid:
        raise ValueError(f"invalid {kind} constructor arity: {len(args)}")
    references(", ".join(args), scope)
    declare(scope, name, kind)
    references(source[ending + 1:], scope)
    where_mode(source[ending + 1:])
    counts.objects += 1


def value(source: str, match: re.Match[str], scope: ChainMap[str, str], counts: Counts) -> None:
    kind, name = match.groups()
    rhs = source[match.end():]
    called = "seg" if kind == "segment" else kind
    if re.match(re.escape(called) + r"\s*\(", rhs):
        raise ValueError(f"same-kind constructor on = rhs: {kind} {name}")
    if kind != "num" and re.match(r"free\b", rhs):
        raise ValueError("free is numeric only")
    before_where = rhs.split("where", 1)[0]
    references(before_where, scope)
    declare(scope, name, kind)
    references(rhs, scope)
    where_mode(rhs)
    counts.objects += 1


def group(source: str, match: re.Match[str], scope: ChainMap[str, str]) -> None:
    name = match.group(1)
    args, ending = arguments(source, match.end() - 1)
    if not args or mask(source[ending + 1:]).strip():
        raise ValueError("group needs members and accepts no suffix")
    for member in args:
        if not REF.fullmatch(member):
            raise ValueError(f"invalid group member: {member}")
        root = member.split(".")[0]
        if root not in scope:
            raise ValueError(f"unknown or forward group member: {member}")
        references(member, scope)
        if scope[root] in {"num", "class", "function"}:
            raise ValueError(f"non-drawable group member: {member}")
    declare(scope, name, "group")


def invoke(source: str, match: re.Match[str], scope: ChainMap[str, str], definitions: dict[str, Definition], counts: Counts) -> None:
    function, name = match.groups()
    definition = definitions.get(function)
    if definition is None:
        raise ValueError(f"unknown, forward, or recursive function: {function}")
    args, ending = arguments(source, match.end() - 1)
    if len(args) != len(definition.params):
        raise ValueError(f"wrong function arity: {function}")
    references(", ".join(args) + source[ending + 1:], scope)
    declare(scope, name, "instance")
    counts.add(definition.counts)


def relation(source: str, scope: ChainMap[str, str], counts: Counts) -> None:
    code = mask(source)
    if code.startswith("rel "):
        solvable(code)
        if "trim_endpoints" in code and re.search(r"\band\b|==", code):
            raise ValueError("clipping is a standalone operation")
    if code.startswith("assert "):
        if re.search(r"\btrim_endpoints\s*\(", code):
            raise ValueError("clipping is not a Boolean assertion")
        counts.assertions += 1
    references(source.split(" ", 1)[1], scope)


def statement(source: str, scope: ChainMap[str, str], definitions: dict[str, Definition], counts: Counts) -> None:
    code = mask(source).strip()
    for pattern, handler in [(CTOR, constructor), (VALUE, value)]:
        match = pattern.match(code)
        if match:
            handler(source, match, scope, counts)
            return
    match = GROUP.match(code)
    if match:
        group(source, match, scope)
    elif code.startswith(("rel ", "assert ")):
        relation(source, scope, counts)
    elif (match := CALL.match(code)):
        invoke(source, match, scope, definitions, counts)
    else:
        raise ValueError("unknown statement")


def global_statement(source: str, scope: ChainMap[str, str]) -> bool:
    code = mask(source)
    if code.startswith("meta "):
        if not re.match(r"meta\s+" + IDENT + r'\s*=\s*"', source):
            raise ValueError("metadata needs one quoted value")
        return True
    if code.startswith("style "):
        if code.strip() != "style textbook":
            raise ValueError("unknown preset")
        return True
    if code.startswith("class "):
        name = re.match(r"class\s+(" + IDENT + r")", code).group(1)
        declare(scope, name, "class")
        references(source[source.index("{"):], scope)
        return True
    return False


def audit(path: Path) -> dict[str, object]:
    source = path.read_text()
    code = mask(source)
    delimiters(code)
    if not source.endswith("\n") or code.splitlines()[0].strip() != "locus 0 beta 6":
        raise ValueError("needs beta 6 header and terminal newline")
    if re.search(r"\b(?:horizontal|vertical|coincident|congruent|pin|offset)\s*\(", code):
        raise ValueError("removed relation vocabulary")
    return audit_lines(path, source)


Active = Optional[tuple[str, Definition, ChainMap[str, str]]]


def process_line(source: str, scope: ChainMap[str, str], definitions: dict[str, Definition], counts: Counts, active: Active) -> Active:
    code = mask(source).strip()
    if (match := FUNCTION.match(code)):
        if active:
            raise ValueError("nested function definition")
        name, raw_params = match.groups()
        if name in BUILTINS | PREDICATES:
            raise ValueError("function shadows a built-in")
        declare(scope, name, "function")
        params = [value.strip() for value in raw_params.split(",")] if raw_params.strip() else []
        if len(set(params)) != len(params) or any(not re.fullmatch(IDENT, param) for param in params):
            raise ValueError("duplicate or invalid parameter")
        return name, Definition(params), scope.new_child(dict.fromkeys(params, "parameter"))
    if code == "}":
        if active is None:
            raise ValueError("orphan function close")
        name, definition, _ = active
        definitions[name] = definition
        return None
    if active:
        statement(source, active[2], definitions, active[1].counts)
    elif not global_statement(source, scope):
        statement(source, scope, definitions, counts)
    return active


def audit_lines(path: Path, source: str) -> dict[str, object]:
    scope = ChainMap(dict(CLASSES))
    definitions: dict[str, Definition] = {}
    counts = Counts()
    active: Active = None
    for number, line in enumerate(source.splitlines()[1:], 2):
        line = line.strip()
        code = mask(line).strip()
        if not code:
            continue
        try:
            active = process_line(line, scope, definitions, counts, active)
        except (ValueError, TypeError) as error:
            raise ValueError(f"line {number}: {error}") from error
    if active:
        raise ValueError("unclosed function definition")
    return {"path": str(path.relative_to(ROOT)), "objects": counts.objects, "assertions": counts.assertions}


def manifest(records: list[dict[str, object]]) -> None:
    stress = {Path(record["path"]).stem: record for record in records if Path(record["path"]).parent == Path("examples/stress")}
    entries = [line.split() for line in (ROOT / "examples/stress/manifest.tsv").read_text().splitlines() if line.strip() and not line.startswith("#")]
    if set(stress) != {entry[0] for entry in entries}:
        raise ValueError("stress manifest does not match primary fixtures")
    for name, objects, assertions, *_ in entries:
        if stress[name]["objects"] < int(objects) or stress[name]["assertions"] < int(assertions):
            raise ValueError(f"stress manifest count minimum not met: {name}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print expanded structural counts")
    options = parser.parse_args()
    paths = sorted((ROOT / "examples/demonstrations").rglob("*.locus")) + sorted((ROOT / "examples/stress").rglob("*.locus"))
    paths.append(ROOT / "tests/fixtures/solver/closed_segment_bisectors.locus")
    records = []
    errors = []
    for path in paths:
        try:
            records.append(audit(path))
        except ValueError as error:
            errors.append(f"{path.relative_to(ROOT)}: {error}")
    if not errors:
        try:
            manifest(records)
        except ValueError as error:
            errors.append(str(error))
    report = {"passed": not errors, "files": records, "errors": errors}
    if options.json:
        print(json.dumps(report, indent=2))
    else:
        print(f"Beta 6 source audit: {len(records)}/{len(paths)} files passed; numerical/layout assertions were not executed.")
        for error in errors:
            print(error)
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
