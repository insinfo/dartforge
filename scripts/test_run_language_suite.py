#!/usr/bin/env python3
"""Regression checks for production-suite execution and trustworthy status files."""
import importlib.util
import json
import shutil
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "language_suite", Path(__file__).with_name("run-language-suite.py"))
suite = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(suite)


def result(code=0, out="ok\n", err=""):
    return {"code": code, "out": out, "err": err}


class ClassificationTests(unittest.TestCase):
    def test_timeouts_and_crashes_never_match(self):
        for code in (-999, -11, -1001):
            r = result(code)
            self.assertIsNone(suite.matches("native", "a", r, {"a": r}, {}))
        for code in (-999, -11, 101, 0):
            r = suite.compile_failure(code, "", "compiler failed")
            self.assertIsNone(suite.matches("native", "a", r, {"a": result(254)}, {}))

    def test_mutual_refusal_is_separate_from_execution(self):
        r = suite.compile_failure(1, "", "unsupported syntax")
        for tool in ("native", "jsprod", "dart2js"):
            self.assertEqual(suite.matches(tool, "a", r, {"a": result(254)}, {}), "rejected")

    def test_production_reference_only_accepts_success(self):
        ref = {"a": result(out="VM type\n")}
        web = result(out="minified type\n")
        self.assertEqual(suite.matches("jsprod", "a", web, ref, {"a": web}), "dart2js")
        failed = result(1, "", "TypeError")
        self.assertIsNone(suite.matches("jsprod", "a", failed, ref, {"a": failed}))
        self.assertIsNone(suite.matches("jsprod", "a", dict(web, err="warning"), ref, {"a": web}))

    def test_identity_hash_output_is_not_erased(self):
        self.assertIsNone(suite.matches("native", "a", result(out="123\n"), {"a": result(out="456\n")}, {}))

    def test_shared_incomplete_async_output_never_passes(self):
        incomplete = result(out="unittest-suite-wait-for-done\n")
        for tool in ("native", "jit", "forge", "wasm", "jsprod", "dart2js", "dartaot"):
            with self.subTest(tool=tool):
                self.assertIsNone(suite.matches(tool, "a", incomplete,
                                               {"a": incomplete}, {"a": incomplete}))
        complete = result(out=incomplete["out"] + "unittest-suite-success\n")
        self.assertEqual(suite.matches("native", "a", complete, {"a": complete}, {}), "vm")


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "sdk/.dart_tool").mkdir(parents=True)
        (self.root / "sdk/.dart_tool/package_config.json").write_text("{}")
        (self.root / "results").mkdir()
        (self.root / "list.txt").write_text("a.dart\nb.dart\n")
        for name, value in (("HERE", str(self.root)), ("SDK", str(self.root / "sdk")),
                            ("RES", str(self.root / "results")), ("WORK", str(self.root / "work"))):
            p = patch.object(suite, name, value)
            p.start()
            self.addCleanup(p.stop)
        self.addCleanup(setattr, suite, "JS_LEVEL", "-O2")
        self.addCleanup(setattr, suite, "STACK_TRACE", None)

    def read(self, name):
        return json.loads((self.root / "results" / name).read_text())

    def reference(self, name="vm", value=None, level="-O2"):
        path = self.root / "results" / (name + ".json")
        path.write_text(json.dumps(
            {"a.dart": value or result(), "b.dart": value or result()}))
        if name == "dart2js":
            profile = {"tool": "dart2js", "jsLevel": level, "dartHash": suite.file_hash(suite.DART),
                       "nodeHash": suite.file_hash(shutil.which(suite.NODE) or suite.NODE),
                       "packagesHash": suite.file_hash(str(self.root / "sdk/.dart_tool/package_config.json"))}
            (self.root / "results/dart2js.run.json").write_text(json.dumps(
                {"state": "complete", "profile": profile, "resultHash": suite.file_hash(str(path))}))

    def test_complete_vm_status_and_journal(self):
        with patch.object(suite, "vm", return_value=result()):
            self.assertEqual(suite.main(["vm", "--jobs", "1"]), 0)
        status = self.read("vm.run.json")
        self.assertEqual(status["state"], "complete")
        self.assertCountEqual(status["selected"], status["completed"])
        self.assertEqual(status["resultHash"], suite.file_hash(str(self.root / "results/vm.json")))
        records = (self.root / "results/vm.journal.jsonl").read_text().splitlines()
        self.assertEqual(len(records), 2)

    def test_vm_failure_returns_nonzero_without_losing_results(self):
        with patch.object(suite, "vm", return_value=result(255, err="Expect failed")):
            self.assertEqual(suite.main(["vm"]), 1)
        self.assertEqual(len(self.read("vm.run.json")["failures"]), 2)
        self.assertEqual(len(self.read("vm.json")), 2)

    def test_vm_async_protocol_failure_preserves_exit_zero_and_journal(self):
        incomplete = result(out="unittest-suite-wait-for-done\n")
        with patch.object(suite, "vm", return_value=incomplete):
            self.assertEqual(suite.main(["vm"]), 1)
        self.assertEqual(len(self.read("vm.run.json")["failures"]), 2)
        self.assertEqual(self.read("vm.json")["a.dart"]["code"], 0)
        records = (self.root / "results/vm.journal.jsonl").read_text().splitlines()
        self.assertTrue(all(json.loads(row)["result"]["code"] == 0 for row in records))

    def test_backend_divergence_returns_nonzero(self):
        self.reference()
        with patch.object(suite, "native", return_value=result(out="wrong\n")):
            self.assertEqual(suite.main(["native"]), 1)
        self.assertEqual(self.read("native.run.json")["exitCode"], 1)

    def test_runner_exception_cannot_write_complete_status(self):
        with patch.object(suite, "vm", side_effect=FileNotFoundError("missing Dart")):
            with self.assertRaises(FileNotFoundError):
                suite.main(["vm"])
        self.assertEqual(self.read("vm.run.json")["state"], "interrupted")

    def test_production_requires_all_reference_results(self):
        self.reference()
        with self.assertRaisesRegex(ValueError, "complete dart2js"):
            suite.main(["jsprod"])
        self.reference("dart2js", level="-O1")
        with patch.object(suite, "jsprod", return_value=result()):
            self.assertEqual(suite.main(["jsprod", "--js-level", "-O1"]), 0)
        self.assertEqual(self.read("jsprod.run.json")["profile"]["jsLevel"], "-O1")
        with self.assertRaisesRegex(ValueError, "same SDK, host and JS level"):
            suite.main(["jsprod", "a.dart"])
        with patch.object(suite, "JSPROD", "different-compiler"), self.assertRaisesRegex(ValueError, "different.*profiles"):
            suite.main(["jsprod", "--js-level", "-O1", "a.dart"])

    def test_invalid_selections_and_unsafe_profiles_are_errors(self):
        for args in (["vm", "no-such-test"], ["vm", "--jobs", "0"],
                     ["jsprod", "--js-level", "-O4"], ["vm", "--unknown"],
                     ["jsprod", "--stack-trace", "symbolic"]):
            with self.subTest(args=args), self.assertRaises(ValueError):
                suite.main(args)

    def test_both_js_backends_use_same_async_host_and_pinned_level(self):
        calls = []
        def fake_run(cmd, cwd, timeout=suite.TIMEOUT):
            calls.append(cmd)
            if "-o" in cmd:
                Path(cmd[cmd.index("-o") + 1]).write_text("console.log('ok')")
            return 0, "ok\n", ""
        with patch.object(suite, "run", side_effect=fake_run):
            for official in (False, True):
                self.assertEqual(suite.production_js("a.dart", official), result())
        compiles = calls[::2]
        self.assertTrue(all("-O2" in cmd for cmd in compiles))
        self.assertIn("--own-sdk", compiles[0])
        self.assertTrue(any(arg.startswith("--packages=") for arg in compiles[1]))
        hosts = list((self.root / "work").rglob("run.cjs"))
        self.assertEqual(len(hosts), 2)
        self.assertEqual(hosts[0].read_text(), hosts[1].read_text())

    def test_native_symbolic_option_reaches_compiler(self):
        suite.STACK_TRACE = "symbolic"
        with patch.object(suite, "run", return_value=(1, "", "refused")) as run:
            suite.native("a.dart")
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[cmd.index("--stack-trace") + 1], "symbolic")


if __name__ == "__main__":
    unittest.main()
