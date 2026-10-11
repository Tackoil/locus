#!/usr/bin/env python3
"""Collect pinned dependency notices; keep unresolved provenance visible."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUPPLEMENTS = ROOT / "licenses" / "third-party" / "sources.json"
OUTPUT = ROOT / "THIRD-PARTY-NOTICES.md"
NOTICE_NAME = re.compile(
    r"^(?:licen[sc]e|copying|copyright|notice|unlicen[sc]e)(?:$|[._-])"
    r"|^authors(?:\.txt|\.md)?$", re.IGNORECASE
)


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def cargo_packages() -> list[dict]:
    packages = {}
    for manifest in (ROOT / "Cargo.toml", ROOT / "ecosystem/zed/Cargo.toml"):
        result = subprocess.run(
            ["cargo", "metadata", "--locked", "--offline", "--all-features",
             "--format-version", "1", "--manifest-path", str(manifest)],
            check=True, capture_output=True, text=True, cwd=ROOT,
        )
        metadata = json.loads(result.stdout)
        members = set(metadata["workspace_members"])
        packages.update({p["id"]: p for p in metadata["packages"] if p["id"] not in members})
    return sorted(packages.values(), key=lambda p: (p["name"], p["version"], p["source"] or ""))


def notice_files(package: dict) -> list[tuple[str, bytes]]:
    root = Path(package["manifest_path"]).parent
    files = {p for p in root.rglob("*") if p.is_file() and NOTICE_NAME.match(p.name)}
    if package.get("license_file"):
        files.add(root / package["license_file"])
    return [(p.relative_to(root).as_posix(), p.read_bytes()) for p in sorted(files)]


def supplemental_files(component: dict, sources: dict) -> list[tuple[str, bytes]]:
    files = []
    for key in component.get("files", []):
        entry = sources["files"][key]
        data = (SUPPLEMENTS.parent / entry["path"]).read_bytes()
        if sha256(data) != entry["sha256"]:
            raise ValueError(f"Supplemental notice changed without provenance update: {key}")
        files.append((entry["origin"], data))
    return files


def package_component(package: dict, sources: dict) -> dict:
    name = f"{package['name']}@{package['version']}"
    supplement = sources["components"].get(name, {})
    files = notice_files(package) + supplemental_files(supplement, sources)
    if not files:
        raise ValueError(f"No notice text found for {name}; investigate before updating")
    return {
        "name": name,
        "source": package["source"] or "path:vendor/static_map_macros",
        "license": package["license"] or supplement.get("license", "UNRESOLVED"),
        "note": supplement.get("note", ""),
        "files": files,
    }


def npm_components(sources: dict) -> list[dict]:
    lock = read_json(ROOT / "ecosystem/tree-sitter-locus/package-lock.json")
    components = []
    for path, package in sorted(lock["packages"].items()):
        if not path:
            continue
        name = f"npm:{path.rsplit('node_modules/', 1)[-1]}@{package['version']}"
        supplement = sources["components"][name]
        if supplement["integrity"] != package["integrity"]:
            raise ValueError(f"NPM integrity changed; recover exact upstream notices for {name}")
        components.append({
            "name": name, "source": package["resolved"], "license": package["license"],
            "note": "Development tool; downloaded native CLI dependencies are outside this npm package inventory.",
            "files": supplemental_files(supplement, sources),
        })
    return components


def font_components(sources: dict) -> list[dict]:
    assets = ROOT / "crates/locus-tex/assets"
    fonts = (
        ("Noto Sans", "OFL.txt", "NotoSans-Regular.ttf"),
        ("Noto Sans CJK SC", "CJK-OFL.txt", "NotoSansCJKsc-Regular.otf"),
        ("STIX Two Math (ReX)", "STIX-OFL.txt", "rex-stix2.otf"),
    )
    return [{
        "name": name, "source": "crates/locus-tex/assets/README.md",
        "license": "OFL-1.1",
        "note": f"Bundled font `{font}`; SHA-256 `{sha256((assets / font).read_bytes())}`.",
        "files": [(f"crates/locus-tex/assets/{filename}", (assets / filename).read_bytes()),
                  ("crates/locus-tex/assets/README.md", (assets / "README.md").read_bytes())]
                 + supplemental_files(sources["components"].get(f"font:{name}", {}), sources),
    } for name, filename, font in fonts]


def fenced(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    longest = max((len(m.group()) for m in re.finditer(r"`+", text)), default=0)
    fence = "`" * max(3, longest + 1)
    return f"{fence}text\n{text.rstrip()}\n{fence}\n"


def component_section(component: dict, texts: dict) -> str:
    lines = [f"## {component['name']}", "", f"Declared license: `{component['license']}`.", "",
             f"Source: `{component['source']}`.", ""]
    if component["note"]:
        lines.extend([component["note"], ""])
    for origin, data in component["files"]:
        digest = sha256(data)
        texts[digest] = data.decode("utf-8")
        lines.append(f"- `{origin}`: [notice text {digest[:16]}](#text-{digest}).")
    return "\n".join(lines) + "\n\n"


def render(sources: dict) -> str:
    packages = cargo_packages()
    components = [package_component(p, sources) for p in packages]
    components += npm_components(sources) + font_components(sources)
    texts = {}
    header = f"""# Third-party notices

Locus's own code is licensed under MIT OR Apache-2.0. Third-party code and
fonts retain their respective licenses, copyright notices, and conditions.

This generated snapshot covers {len(packages)} Rust package versions from both
locked workspaces, all features and target platforms, including development and
build dependencies; the locked npm packages; and the three bundled fonts.
Inclusion does not mean every component is shipped in every Locus binary.
Rust compiler/standard-library notices and tools installed separately are outside
this inventory. Keep their notices too when distributing those components.

For alternatives joined by OR (including legacy slash notation), a compatible
permissive alternative may be selected; AND obligations remain cumulative.
Reproducing all supplied license files does not select every alternative.

**ReX provenance is not fully resolved.** Its README declares MIT/Apache-2.0
with unspecified BSD-like portions, but omits the referenced license files.
Its generated tables also cite unicode-math. Read
[the investigation](licenses/REX-LICENSING.md) before distributing a ReX build.
The current CLI enables ReX in its ordinary dependency configuration.

Regenerate with `python3 scripts/generate_third_party_notices.py`; use `--check`
to detect drift. Populate dependency caches with `cargo fetch --locked` and
`cargo fetch --locked --manifest-path ecosystem/zed/Cargo.toml` first.
Supplemental upstream texts and their checksums are recorded in
`licenses/third-party/sources.json`. Do not replace missing notices with an
assumed license or infer a license from repository visibility.

"""
    sections = "".join(component_section(c, texts) for c in components)
    licenses = "".join(
        f'<a id="text-{digest}"></a>\n\n## Notice text {digest[:16]}\n\n{fenced(text)}\n'
        for digest, text in sorted(texts.items())
    )
    return header + sections + licenses


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if committed notices are stale")
    args = parser.parse_args()
    content = render(read_json(SUPPLEMENTS))
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != content:
            raise SystemExit("Third-party notices are stale; regenerate them")
        print("Third-party notices match the locked dependencies and source texts.")
        return
    OUTPUT.write_text(content, encoding="utf-8")
    print(f"Wrote {OUTPUT.name} ({len(content.encode('utf-8'))} bytes)")


if __name__ == "__main__":
    main()
