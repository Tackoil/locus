"""Offline checks for billing evidence and the task's independent gate."""

import json
import io
import os
import sys
from contextlib import redirect_stdout
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

import hashlib
import benchmark
from run import REPOSITORY, SKILL, execute
from experiment import (PROTOCOL, completed_trial, initialize, require_version,
                        save_rating_template, trial_plan)
from workflow_report import criterion_met
from harnesses import write_json
from meter import ProxyHandler, extract_usage, request_rates, summarize
from report import rating_total
from svg_validation import check_svg, snapshot_svg
from sequence_contract import MESSAGE_IDS, MESSAGE_TEXT, PARTICIPANTS
from validation import attempt_records, completed_scene_checks, probe, repair_metrics, validate
from validation import read_jsonl

ROOT = REPOSITORY
BENCHMARK = Path(__file__).resolve().parent


class DirectSvgTests(unittest.TestCase):
    def test_gate_rejects_missing_malformed_and_non_svg(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "figure.svg"
            self.assertFalse(check_svg(source)["passed"])
            for text in ("<svg", "<html/>"):
                source.write_text(text)
                self.assertFalse(check_svg(source)["passed"])
            source.write_text('<svg xmlns="http://www.w3.org/2000/svg"/>')
            self.assertTrue(check_svg(source)["passed"])

    def test_observed_revisions_preserve_bytes_and_deduplicate(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            source = folder / "figure.svg"
            attempts = folder / "attempts"
            first = b'<svg xmlns="http://www.w3.org/2000/svg"><text>first</text></svg>'
            source.write_bytes(first)
            snapshot_svg(source, attempts)
            snapshot_svg(source, attempts)
            source.write_bytes(first.replace(b"first", b"revised"))
            snapshot_svg(source, attempts)
            rows = read_jsonl(attempts / "checks.jsonl")
            self.assertEqual(repair_metrics(rows)["retry_count"], 1)
            self.assertEqual((attempts / (rows[0]["sha256"] + ".svg")).read_bytes(), first)


class AccountingTests(unittest.TestCase):
    def test_request_capture_preserves_wire_settings_without_the_credential(self):
        with tempfile.TemporaryDirectory() as directory:
            records, forwarded = [], []
            handler = ProxyHandler.__new__(ProxyHandler)
            handler.server = SimpleNamespace(reserve=lambda: 1, max_requests=64, observer=None,
                                            model="deepseek-flash", key="test-secret",
                                            folder=Path(directory), save=records.append)
            body = json.dumps({"model": "other", "stream": True,
                               "messages": [{"role": "user", "content": "test-secret"}]}).encode()
            handler.headers = {"Content-Length": str(len(body))}
            handler.path = "/v1/chat/completions"
            handler.rfile = io.BytesIO(body)
            handler.forward = lambda data, record, stream: forwarded.append(data)
            handler.do_POST()
            wire = json.loads(forwarded[0])
            captured = json.loads((Path(directory) / "requests/0001.json").read_text())
            self.assertEqual(wire["model"], "deepseek-flash")
            self.assertEqual(wire["max_tokens"], PROTOCOL["max_output_tokens"])
            self.assertEqual(wire["thinking"], {"type": "disabled"})
            self.assertEqual(wire["temperature"], 0)
            self.assertEqual(captured["messages"][0]["content"], "[REDACTED_API_KEY]")
            self.assertEqual(records[0]["request_sha256"], hashlib.sha256(forwarded[0]).hexdigest())

    def test_cached_input_is_not_counted_twice(self):
        usage = {"prompt_tokens": 1000, "prompt_cache_hit_tokens": 600,
                 "prompt_cache_miss_tokens": 400, "completion_tokens": 200,
                 "completion_tokens_details": {"reasoning_tokens": 80}}
        rates = request_rates("2026-10-07T10:00:00+08:00")
        record = {"status": 200, "usage": usage, "rates": rates, "request_sha256": "one"}
        result = summarize([record, record])
        self.assertEqual(result["total_tokens"], 2400)
        self.assertEqual(result["cache_hit_tokens"], 1200)
        self.assertEqual(result["api_repeated_payloads"], 1)
        self.assertAlmostEqual(result["estimated_cost_cny"], 0.004848)

    def test_missing_usage_is_unknown_cost(self):
        result = summarize([{"status": 200, "usage": None, "request_sha256": "one"}])
        self.assertFalse(result["usage_complete"])
        self.assertIsNone(result["estimated_cost_cny"])

    def test_lost_response_does_not_claim_complete_billing(self):
        records = [{"status": 200, "usage": {"prompt_tokens": 10, "completion_tokens": 1},
                    "rates": request_rates("2026-10-07T10:00:00+08:00"), "request_sha256": "one"},
                   {"status": 0, "usage": None, "request_sha256": "two"}]
        result = summarize(records)
        self.assertFalse(result["usage_complete"])
        self.assertIsNone(result["estimated_cost_cny"])

    def test_no_provider_requests_have_zero_api_cost(self):
        self.assertEqual(summarize([])["estimated_cost_cny"], 0)

    def test_peak_boundaries_and_weekend(self):
        for moment in ("2026-10-07T12:00:00+08:00", "2026-10-07T18:00:00+08:00",
                       "2026-10-10T10:00:00+08:00"):
            self.assertEqual(request_rates(moment)["period"], "off_peak")
        self.assertEqual(request_rates("2026-10-07T14:00:00+08:00")["period"], "peak")

    def test_streamed_usage_and_done(self):
        line = b'data: {"choices": [], "usage": {"prompt_tokens": 2, "completion_tokens": 3}}\n'
        self.assertEqual(extract_usage(line)["completion_tokens"], 3)
        self.assertIsNone(extract_usage(b"data: [DONE]\n"))
        self.assertIsNone(extract_usage(b": heartbeat\n"))

    def test_rechecks_do_not_count_as_repairs(self):
        rows = [{"sha256": "first", "exit_code": 1}, {"sha256": "first", "exit_code": 1},
                {"sha256": "fixed", "exit_code": 0}, {"sha256": "fixed", "exit_code": 0}]
        self.assertEqual(repair_metrics(rows), {"checked_versions": 2, "retry_count": 1,
                                               "failed_versions": 1})

    def test_native_absolute_cli_checks_are_observable(self):
        messages = [{"role": "assistant", "tool_calls": [
            {"id": "checked", "function": {"arguments": json.dumps({
                "command": "/some/path/locus check --json figure.locus"})}},
            {"id": "pending", "function": {"arguments": json.dumps({
                "command": "locus check figure.locus"})}}]},
            {"role": "tool", "tool_call_id": "checked", "content": "passed"}]
        self.assertEqual(completed_scene_checks(messages), {"checked"})

    def test_forward_stream_preserves_bytes_and_captures_provider_usage(self):
        body = (b'data: {"choices":[{"delta":{"content":"hello"}}]}\n\n'
                b'data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}\n\n'
                b'data: [DONE]\n\n')
        upstream = io.BytesIO(body)
        upstream.status, upstream.headers = 200, {"Content-Type": "text/event-stream"}
        handler = ProxyHandler.__new__(ProxyHandler)
        handler.server = SimpleNamespace(upstream="https://api.deepseek.com", key="test-secret")
        handler.wfile = io.BytesIO()
        handler.send_response = lambda status: None
        handler.send_header = lambda *args: None
        handler.end_headers = lambda: None
        record = {"usage": None}
        with patch("meter.urlopen", return_value=upstream) as connection:
            handler.forward(b'{"stream":true}', record, True)
        self.assertEqual(handler.wfile.getvalue(), body)
        self.assertEqual(record["usage"], {"prompt_tokens": 10, "completion_tokens": 5})
        self.assertEqual(connection.call_args.args[0].full_url,
                         "https://api.deepseek.com/chat/completions")
        self.assertNotIn("test-secret", json.dumps(record))


class GeometryContractTests(unittest.TestCase):
    def setUp(self):
        self.binary = ROOT / "target/debug/locus"
        if not self.binary.exists():
            self.skipTest("Build locus-cli before geometry contract tests")
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.source = Path(self.folder.name) / "figure.locus"

    def test_unrelated_valid_figure_cannot_pass(self):
        self.source.write_text("locus 0 beta 4\npoint P = point()\n")
        code, report = validate(self.binary, self.source, BENCHMARK / "tasks/triangle-contract.locus")
        self.assertNotEqual(code, 0)
        self.assertFalse(report["passed"])

    def test_independent_gate_rejects_wrong_dimensions(self):
        source = """locus 0 beta 4
point A = point()
point B = point() where { horizontal(A, B), hdist(A, B) == 4 }
point C = point() where { vertical(A, C), vdist(A, C) == 3 }
polygon tri = polygon(A, B, C)
point H = foot(A, line(B, C))
segment altitude = seg(A, H) [highlight]
circle circum = circumcircle(A, B, C) [construction]
circle inc = incircle(A, B, C) [auxiliary]
right_angle rA = right_angle(A, B, C)
right_angle rH = right_angle(H, A, B)
label lA = label(A, "A")
label lB = label(B, "B")
label lC = label(C, "C")
label lH = label(H, "H")
text_block note = text_block("Area = 6; altitude = 2.4")
assert area(tri) > 0
assert radius(inc) > 0
assert radius(circum) > 0
assert dist(A, B) > 0
assert dist(A, C) > 0
assert length(altitude) > 0
"""
        self.source.write_text(source)
        code, report = validate(self.binary, self.source, BENCHMARK / "tasks/triangle-contract.locus")
        self.assertEqual(code, 0, json.dumps(report))
        self.source.write_text(source.replace("hdist(A, B) == 4", "hdist(A, B) == 8"))
        code, report = validate(self.binary, self.source, BENCHMARK / "tasks/triangle-contract.locus")
        self.assertNotEqual(code, 0)
        self.assertFalse(report["passed"])


class RatingTests(unittest.TestCase):
    def test_rating_template_cannot_be_reported_as_inspected(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / "pi-1").mkdir()
            svg = folder / "pi-1/figure.svg"
            svg.write_text('<svg xmlns="http://www.w3.org/2000/svg"/>')
            result = {"harness": "pi", "repetition": 1, "svg": str(svg),
                      "svg_sha256": hashlib.sha256(svg.read_bytes()).hexdigest()}
            save_rating_template(folder, result)
            ratings = json.loads((folder / "ratings.json").read_text())
            self.assertIsNone(rating_total(ratings["pi-1"], result, folder))

    def test_removed_successful_svg_is_not_silently_scored_zero(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "Declared SVG is missing"):
                rating_total({}, {"harness": "pi", "repetition": 1, "svg": "present-before"}, Path(directory))

    def test_uninspected_image_stays_unrated(self):
        self.assertIsNone(rating_total(None, {}, Path("unused")))

    def test_stale_image_assessment_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "pi-1").mkdir()
            (root / "pi-1/figure.svg").write_text('<svg xmlns="http://www.w3.org/2000/svg"/>')
            with self.assertRaisesRegex(ValueError, "does not match"):
                rating_total({"svg_sha256": "stale"}, {"harness": "pi", "repetition": 1}, root)


def sequence_gate_fixture():
    statements = ["locus 0 beta 4", "rect customer = rect(5, 1)"]
    previous = "customer"
    for name in ("web", "orders", "payment"):
        statements.append(f"rect {name} = rect(5, 1) where {{ horizontal({previous}.center, {name}.center), "
                          f"hdist({previous}.center, {name}.center) == 10 }}")
        previous = name
    for name, title in PARTICIPANTS.items():
        statements.extend([f'label h_{name} = label({name}.center, "{title}") {{ dir: center }}',
                           f'segment life_{name} = seg({name}.s, offset({name}.s, 0, -10))'])
    statements.extend(["rect act_web = rect(web.center, 0.2, 5)",
                       "rect act_orders = rect(orders.center, 0.2, 5)",
                       "rect alt_frame = rect(orders.center, 30, 8)"])
    for name, text in zip(MESSAGE_IDS, MESSAGE_TEXT):
        statements.extend([f'arrow {name} = arrow(customer.s, web.s)',
                           f'label l_{name} = label(customer.center, "{text}")'])
    statements.extend(['label approved = label(customer.center, "[payment approved]")',
                       'label declined = label(customer.center, "[payment declined]")'])
    statements.extend([f"assert dist(customer.center, {name}.center) > 0" for name in ("web", "orders", "payment")])
    statements.extend([f"assert length(life_{name}) > 0" for name in ("customer", "web", "orders")])
    return "\n".join(statements) + "\n"


class ExperimentTests(unittest.TestCase):
    def test_output_cannot_create_generated_artifacts_inside_the_benchmark(self):
        args = SimpleNamespace(output=BENCHMARK / "must-not-exist")
        with self.assertRaisesRegex(ValueError, "under ignored target"):
            initialize(args, {})
        self.assertFalse(args.output.exists())

    def test_interrupt_terminates_native_processes_before_preserving_the_partial_run(self):
        with tempfile.TemporaryDirectory() as directory:
            process = SimpleNamespace(pid=424242, wait=Mock(side_effect=[KeyboardInterrupt, 0]))
            with patch("run.subprocess.Popen", return_value=process), patch("run.os.killpg") as stop:
                with self.assertRaises(KeyboardInterrupt):
                    execute(["offline-fixture"], Path(directory), {}, 1)
            stop.assert_called_once()
            self.assertEqual(stop.call_args.args[0], process.pid)

    def test_matrix_resume_and_report_use_saved_results_without_new_model_calls(self):
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(output=Path(directory) / "matrix", resume=False,
                                   tasks=["triangle"], harnesses=["pi"], runs=1, max_requests=64, timeout=600)
            with patch("benchmark.preflight", return_value=({"pi": ["unused"]}, {"test": "identity"})), \
                 patch("run.credentials", return_value="offline-placeholder"), \
                 patch("run.metadata", side_effect=matrix_metadata), \
                 patch("run_svg.metadata", side_effect=lambda args, task, commands: matrix_metadata(args, commands)), \
                 patch("benchmark.execute_trial", side_effect=matrix_trial) as execute:
                with redirect_stdout(io.StringIO()):
                    benchmark.run_matrix(args)
                    args.resume = True
                    benchmark.run_matrix(args)
                self.assertEqual(execute.call_count, 2)
            with self.assertRaisesRegex(ValueError, "Inspect and rate"):
                benchmark.report_matrix(args)
            for backend in ("locus", "svg"):
                path = args.output / backend / "triangle/ratings.json"
                ratings = json.loads(path.read_text())
                ratings["pi-1"].update(reviewer="Offline test fixture", observations="Synthetic rating for report integration",
                                       scores={"geometry": 40, "completeness": 25, "readability": 20, "styling": 15})
                write_json(path, ratings)
            with redirect_stdout(io.StringIO()):
                benchmark.report_matrix(args)
            grouped = json.loads((args.output / "workflow-summary.json").read_text())
            self.assertEqual(len(grouped["groups"]), 2)
            self.assertFalse(grouped["canonical"])

    def test_plan_contains_every_pair_and_alternates_workflow_order(self):
        plan = trial_plan(PROTOCOL["tasks"], PROTOCOL["harnesses"], 3)
        self.assertEqual(len(plan), 36)
        self.assertEqual(len({tuple(t.values()) for t in plan}), 36)
        for harness in PROTOCOL["harnesses"]:
            starts = [t["backend"] for i, t in enumerate(plan)
                      if i % 2 == 0 and t["harness"] == harness and t["task"] == "triangle"]
            self.assertNotEqual(starts[0], starts[1])
            self.assertEqual(starts[0], starts[2])

    def test_version_drift_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "expected 0.84.4"):
            require_version("pi", "0.84.5", "0.84.4")

    def test_resume_rejects_changed_inputs_and_preserves_completed_trials(self):
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(output=Path(directory) / "experiment", resume=False,
                                   tasks=["triangle"], harnesses=["pi"], runs=1, max_requests=64, timeout=600)
            initialize(args, {"fixture": "identity"})
            frozen_skill = args.output / "inputs/skill"
            self.assertEqual({p.relative_to(frozen_skill) for p in frozen_skill.rglob("*") if p.is_file()},
                             {p.relative_to(SKILL) for p in SKILL.rglob("*") if p.is_file()})
            self.assertTrue((frozen_skill / "SKILL.md").is_file())
            self.assertFalse((frozen_skill / "benchmark").exists())
            args.resume = True
            with patch("experiment.input_hashes", return_value={"changed": "digest"}):
                with self.assertRaisesRegex(ValueError, "input_hashes changed"):
                    initialize(args, {"fixture": "identity"})
            initialize(args, {"fixture": "identity"})
            frozen = args.output / "inputs/benchmark/tasks/triangle-locus.txt"
            frozen.write_text("modified")
            with self.assertRaisesRegex(ValueError, "Frozen input modified"):
                initialize(args, {"fixture": "identity"})

    def test_partial_trial_requires_explicit_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / "raw/pi-1").mkdir(parents=True)
            with self.assertRaisesRegex(ValueError, "Interrupted trial"):
                completed_trial(folder, {"harness": "pi", "repetition": 1})

    def test_cheaper_incomplete_or_worse_images_do_not_meet_the_criterion(self):
        baseline = {"runs": 3, "exported_svgs": 3, "total_tokens": 100,
                    "estimated_cost_cny": 1, "mean_correctness": 40,
                    "mean_completeness": 25, "mean_visual_score": 95}
        cheaper = {**baseline, "total_tokens": 90, "estimated_cost_cny": 0.9}
        self.assertTrue(criterion_met(cheaper, baseline))
        for change in ({"exported_svgs": 2}, {"mean_correctness": 39}, {"estimated_cost_cny": None}):
            self.assertFalse(criterion_met({**cheaper, **change}, baseline))


def matrix_metadata(args, commands):
    return {"model": "deepseek-flash", "runs_per_harness": args.runs,
            "pricing_checked_on": "2026-10-07", "results": []}


def matrix_trial(args, trial, folder, key, commands):
    destination = folder / f"{trial['harness']}-{trial['repetition']}"
    destination.mkdir()
    svg = destination / "figure.svg"
    svg.write_text('<svg xmlns="http://www.w3.org/2000/svg"/>')
    result = {**trial, "svg": str(svg), "svg_sha256": hashlib.sha256(svg.read_bytes()).hexdigest(),
              "exit_code": 0, "timed_out": False, "elapsed_seconds": 1, "total_tokens": 2,
              "api_requests": 1, "estimated_cost_cny": 0.01, "retry_count": 0,
              "contract_passed": True, "harness_exported_svg": True}
    write_json(destination / "metrics.json", result)
    return result


class SequenceContractTests(unittest.TestCase):
    def test_native_wrapper_keeps_its_logs_inside_the_workspace(self):
        binary = ROOT / "target/debug/locus"
        if not binary.exists():
            self.skipTest("Build locus-cli before wrapper tests")
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            work = root / "work"
            work.mkdir()
            source = work / "figure.locus"
            source.write_text(sequence_gate_fixture())
            outside = root / "outside-workspace"
            environment = {"LOCUS_REAL_BIN": str(binary), "LOCUS_BENCH_SOURCE": str(source),
                           "LOCUS_BENCH_CONTRACT": str(BENCHMARK / "tasks/sequence-contract.locus"),
                           "LOCUS_BENCH_ATTEMPTS": str(outside)}
            with patch.dict(os.environ, environment), patch.object(sys, "argv", ["validation.py", "check", str(source)]):
                with redirect_stdout(io.StringIO()):
                    self.assertEqual(probe(), 0)
            self.assertTrue((work / ".benchmark-attempts/checks.jsonl").is_file())
            self.assertFalse(outside.exists())

    def test_sandbox_check_records_merge_without_double_counting(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ("attempts", "work/.benchmark-attempts"):
                path = root / name
                path.mkdir(parents=True)
                (path / "checks.jsonl").write_text(json.dumps({"sha256": "first", "exit_code": 0}) + "\n")
            result = repair_metrics(attempt_records(root))
            self.assertEqual(result["checked_versions"], 1)
            self.assertEqual(result["retry_count"], 0)

    def test_arrangement_gate_catches_swapped_participants(self):
        binary = ROOT / "target/debug/locus"
        if not binary.exists():
            self.skipTest("Build locus-cli before arrangement tests")
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "figure.locus"
            fixture = sequence_gate_fixture()
            contract = BENCHMARK / "tasks/sequence-contract.locus"
            source.write_text(fixture)
            code, result = validate(binary, source, contract)
            self.assertEqual(code, 0, json.dumps(result))
            source.write_text(fixture.replace("hdist(customer.center, web.center) == 10",
                                              "hdist(customer.center, web.center) == -10"))
            code, result = validate(binary, source, contract)
            self.assertNotEqual(code, 0)
            self.assertFalse(result["passed"])

    def test_required_message_text_is_enforced(self):
        binary = ROOT / "target/debug/locus"
        if not binary.exists():
            self.skipTest("Build locus-cli before arrangement tests")
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "figure.locus"
            source.write_text(sequence_gate_fixture().replace('"paymentDeclined"', '"wrongResponse"'))
            code, result = validate(binary, source, BENCHMARK / "tasks/sequence-contract.locus")
            self.assertNotEqual(code, 0)
            self.assertTrue(any("paymentDeclined" in error for error in result["benchmark_errors"]))


if __name__ == "__main__":
    unittest.main()
