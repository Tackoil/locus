"""Offline protocol and evidence checks; no agent/provider calls."""

import argparse
import contextlib
import io
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import benchmark as bench


SVG = '<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0 L1 1"/></svg>'


class BenchmarkTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.protocol = bench.read_json(bench.HERE / "protocol.json")

    def fixture(self, workflow="svg"):
        output = self.root / "experiment"
        trial = {"id": f"codex-triangle-{workflow}-1", "task": "triangle", "workflow": workflow}
        folder = output / "trials" / trial["id"]
        work = folder / "work"
        work.mkdir(parents=True)
        (work / "figure.svg").write_text(SVG)
        (folder / "prompt.txt").write_text("frozen prompt")
        trial["prompt_sha256"] = bench.digest(folder / "prompt.txt")
        binary = self.root / "locus"
        binary.write_text("stub executable identity")
        inputs = output / "inputs"
        inputs.mkdir()
        frozen = inputs / "protocol.json"
        bench.write_json(frozen, self.protocol)
        manifest = {"protocol": self.protocol, "trials": [trial], "locus": bench.identity(binary),
                    "input_hashes": {"inputs/protocol.json": bench.digest(frozen)}}
        bench.write_json(output / "manifest.json", manifest)
        args = argparse.Namespace(output=str(output), trial=trial["id"],
                                  status="completed", elapsed_seconds=12.5)
        return output, work, manifest, args

    def record(self, args):
        with contextlib.redirect_stdout(io.StringIO()):
            bench.record(args)

    def test_schedule_pairs_every_task_and_alternates(self):
        trials = bench.balanced_trials(self.protocol, 2, "codex")
        self.assertEqual(len(trials), 12)
        self.assertEqual(len({t["id"] for t in trials}), 12)
        self.assertEqual([t["workflow"] for t in trials[:2]], ["locus", "svg"])
        self.assertEqual([t["workflow"] for t in trials[6:8]], ["svg", "locus"])

    def test_svg_requires_namespace_and_valid_xml(self):
        source = self.root / "test.svg"
        for content in ("<svg/>", "<svg", '<!DOCTYPE svg><svg xmlns="http://www.w3.org/2000/svg"/>'):
            source.write_text(content)
            self.assertFalse(bench.check_svg(source)["passed"])
        source.write_text(SVG)
        self.assertTrue(bench.check_svg(source)["passed"])

    def test_svg_parse_success_has_limited_scope(self):
        output, _, manifest, args = self.fixture()
        self.record(args)
        metrics = bench.completed_metrics(output, manifest)[0][1]
        self.assertTrue(metrics["svg"]["passed"])
        self.assertEqual(metrics["native"]["scope"], "XML syntax and SVG root only")
        self.assertIsNone(metrics["tokens"])
        self.assertIsNone(metrics["cost"])
        self.assertIsNone(metrics["revisions"])

    def test_records_failure_without_export(self):
        output, work, manifest, args = self.fixture()
        (work / "figure.svg").unlink()
        args.status = "timeout"
        self.record(args)
        metrics = bench.completed_metrics(output, manifest)[0][1]
        self.assertEqual(metrics["status"], "timeout")
        self.assertIsNone(metrics["svg_sha256"])
        self.assertFalse(metrics["svg"]["passed"])

    def test_rejects_overwrite(self):
        _, _, _, args = self.fixture()
        self.record(args)
        with self.assertRaisesRegex(ValueError, "already recorded"):
            self.record(args)

    def test_rejects_negative_and_nonfinite_elapsed(self):
        _, _, _, args = self.fixture()
        for elapsed in (-1, float("nan"), float("inf")):
            args.elapsed_seconds = elapsed
            with self.assertRaisesRegex(ValueError, "Elapsed"):
                self.record(args)

    def test_input_and_binary_drift_rejected(self):
        output, _, manifest, _ = self.fixture()
        source = output / "inputs/protocol.json"
        original = source.read_bytes()
        source.write_text("changed")
        with self.assertRaisesRegex(ValueError, "Frozen input changed"):
            bench.load_experiment(output)
        source.write_bytes(original)
        Path(manifest["locus"]["path"]).write_text("changed executable")
        with self.assertRaisesRegex(ValueError, "executable changed"):
            bench.load_experiment(output)

    def test_prompt_drift_rejected(self):
        output, _, _, args = self.fixture()
        (output / "trials" / args.trial / "prompt.txt").write_text("changed")
        with self.assertRaisesRegex(ValueError, "prompt changed"):
            self.record(args)

    def test_svg_mutation_or_removal_rejected(self):
        output, work, manifest, args = self.fixture()
        self.record(args)
        for content in ("changed", None):
            if content is None:
                (work / "figure.svg").unlink()
            else:
                (work / "figure.svg").write_text(content)
            with self.assertRaisesRegex(ValueError, "SVG changed"):
                bench.completed_metrics(output, manifest)

    def test_locus_source_mutation_rejected(self):
        output, work, manifest, args = self.fixture("locus")
        source = work / "figure.locus"
        source.write_text("original source")
        with patch.object(bench, "validate_locus", return_value={"passed": False}):
            self.record(args)
        source.write_text("changed source")
        with self.assertRaisesRegex(ValueError, "Locus source changed"):
            bench.completed_metrics(output, manifest)

    def test_trial_skill_copy_must_match_frozen_input(self):
        output, work, manifest, args = self.fixture("locus")
        skill = output / "inputs/skill/SKILL.md"
        skill.parent.mkdir()
        skill.write_text("frozen skill")
        manifest["input_hashes"]["inputs/skill/SKILL.md"] = bench.digest(skill)
        bench.write_json(output / "manifest.json", manifest)
        (work / "skill").mkdir()
        (work / "skill/SKILL.md").write_text("changed copy")
        with self.assertRaisesRegex(ValueError, "Trial skill differs"):
            self.record(args)

    def test_timeout_retains_partial_command_output(self):
        error = subprocess.TimeoutExpired(["locus"], 60, output=b"partial", stderr=b"diagnostic")
        with patch.object(bench.subprocess, "run", side_effect=error):
            result = bench.command(["locus"])
        self.assertIsNone(result["exit_code"])
        self.assertTrue(result["timed_out"])
        self.assertEqual(result["stdout"], "partial")
        self.assertEqual(result["stderr"], "diagnostic")

    def test_blind_review_rerenders_svg_without_trusting_agent_png(self):
        output, work, manifest, args = self.fixture()
        self.record(args)
        tasks = output / "inputs/tasks"
        tasks.mkdir()
        (tasks / "triangle.txt").write_text("task")
        (work / "preview.png").write_text("untrusted stale image")
        manifest["renderer"] = {"node": "node", "sharp": "sharp"}
        bench.write_json(output / "manifest.json", manifest)

        def render(argv, cwd=None, env=None):
            self.assertEqual(bench.digest(Path(argv[2])), bench.digest(work / "figure.svg"))
            Path(argv[3]).write_text("independently rendered")
            return {"exit_code": 0}

        with patch.object(bench, "command", side_effect=render):
            with contextlib.redirect_stdout(io.StringIO()):
                bench.review(args)
        preview = next((output / "review").glob("*.png"))
        self.assertEqual(preview.read_text(), "independently rendered")
        self.assertNotIn("workflow", (output / "review/ratings.json").read_text())

    def test_native_output_must_match_original_export(self):
        output, work, manifest, args = self.fixture("locus")
        (work / "figure.locus").write_text("source")

        def native(argv, cwd=None):
            if "render" in argv:
                (output / "trials" / args.trial / "verified.svg").write_text("different output")
            return {"exit_code": 0}

        with patch.object(bench, "command", side_effect=native):
            self.record(args)
        metrics = bench.completed_metrics(output, manifest)[0][1]
        self.assertFalse(metrics["native"]["passed"])
        self.assertFalse(metrics["native"]["svg_matches_source"])

    def test_rating_requires_hash_and_valid_component_scores(self):
        metrics = {"svg_sha256": "expected"}
        rating = {"svg_sha256": "expected", "reviewer": "image reviewer", "observations": "Inspected PNG",
                  "scores": self.protocol["rubric"].copy()}
        self.assertEqual(bench.rating_score(rating, metrics, self.protocol["rubric"]), 100)
        for value in (41, -1, float("nan"), True, None):
            rating["scores"]["geometry"] = value
            with self.assertRaises(ValueError):
                bench.rating_score(rating, metrics, self.protocol["rubric"])
        rating["svg_sha256"] = "stale"
        with self.assertRaisesRegex(ValueError, "Stale rating"):
            bench.rating_score(rating, metrics, self.protocol["rubric"])

    def test_report_requires_recorded_trials_and_review_when_requested(self):
        output, _, _, args = self.fixture()
        args.require_reviewed = True
        with self.assertRaises(FileNotFoundError):
            bench.report(args)
        self.record(args)
        with self.assertRaisesRegex(ValueError, "Unreviewed"):
            bench.report(args)
        args.require_reviewed = False
        with contextlib.redirect_stdout(io.StringIO()):
            bench.report(args)
        report = (output / "report.md").read_text()
        self.assertIn("unreviewed", report)
        self.assertIn("No token-efficiency or cost claim", report)


if __name__ == "__main__":
    unittest.main()
