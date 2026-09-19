#!/usr/bin/env python3
"""Stage a portable dev extension using a snapshot of the local grammar."""

import json
from pathlib import Path
import shutil
import subprocess


ROOT = Path(__file__).resolve().parent
GRAMMAR = ROOT.parent / "tree-sitter-locus"
OUTPUT = ROOT / "target" / "dev-extension"
SNAPSHOT = ROOT / "target" / "grammar-source"


def git(*arguments):
    """Run Git only inside the generated grammar snapshot."""
    return subprocess.run(
        ["git", "-C", str(SNAPSHOT), "-c", "core.hooksPath=/dev/null", *arguments],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


def snapshot_grammar():
    """Zed fetches grammar sources from Git, including for local extensions."""
    if not (GRAMMAR / "src" / "parser.c").is_file():
        raise SystemExit("Generate the parser first: cd ecosystem/tree-sitter-locus && npm ci && npm run generate")
    SNAPSHOT.mkdir(parents=True, exist_ok=True)
    shutil.copytree(GRAMMAR / "src", SNAPSHOT / "src", dirs_exist_ok=True)
    for name in ["grammar.js", "tree-sitter.json", "package.json"]:
        shutil.copy2(GRAMMAR / name, SNAPSHOT / name)
    git("init", "--quiet")
    git("add", ".")
    if git("diff", "--cached", "--name-only"):
        git("-c", "user.name=Locus grammar build", "-c", "user.email=build@localhost",
            "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Snapshot local Locus grammar")
    return git("rev-parse", "HEAD")


def main():
    revision = snapshot_grammar()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for name in ["Cargo.toml", "Cargo.lock"]:
        shutil.copy2(ROOT / name, OUTPUT / name)
    for name in ["src", "languages"]:
        shutil.copytree(ROOT / name, OUTPUT / name, dirs_exist_ok=True)
    manifest = (ROOT / "extension.toml").read_text()
    manifest += (
        "\n[grammars.locus]\n"
        f"repository = {json.dumps(SNAPSHOT.as_uri())}\n"
        f"rev = {json.dumps(revision)}\n"
    )
    (OUTPUT / "extension.toml").write_text(manifest)
    print(f"In Zed, run 'zed: install dev extension' and select:\n{OUTPUT}")


if __name__ == "__main__":
    main()
