"""Editing comparison failures with real child processes and no installed SDK."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import editing_compare


class EditingCompareTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="dartforge-editing-compare-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.out = self.root / "results"
        self.out.mkdir()
        self.driver = self.root / "fake driver.py"
        self.driver.write_text('''import json, sys
from pathlib import Path
args = sys.argv[1:]
path = Path(args[args.index('--json') + 1])
label = args[args.index('--label') + 1]
mode = args[args.index('--mode') + 1]
if mode == 'exit':
    print('driver failure', flush=True)
    raise SystemExit(7)
if mode == 'missing':
    raise SystemExit(0)
if mode == 'invalid':
    path.write_text('{broken')
    raise SystemExit(0)
if mode == 'shape':
    path.write_text('{}')
    raise SystemExit(0)
memory = {key: 1 for key in (
    'total_peak_mib', 'total_mean_mib', 'total_rest_mib', 'rss_peak_mib')}
path.write_text(json.dumps({
    'latency_ms': {}, 'memory': memory, 'diagnostics': {},
    'api_insertion_byte': int(args[args.index('--api-offset') + 1]),
    'label': label,
}))
''', encoding="utf-8")
        self.fresh = self.out / "sample.after.0.json"
        self.log = self.out / "sample.after.0.log"
        self.old = json.dumps({"latency_ms": {}, "memory": {
            key: 1 for key, _ in editing_compare.MEMORY}, "diagnostics": {}, "old": True})

    def seed_previous(self):
        self.fresh.write_text(self.old, encoding="utf-8")
        self.log.write_text("old log", encoding="utf-8")

    def invoke(self, mode="valid", options=None, launch_error=False):
        argv = ["editing_compare.py", "--driver", str(self.driver),
                "--server", "after=unused", "--dart", "unused", "--rounds", "1",
                "--out", str(self.out), "--project",
                f"name=sample,root={self.root},file=lib/a.dart,api=lib/b.dart,api-offset=17"]
        argv += ["--only", "after"] if options is None else options
        argv += ["--", "--mode", mode]
        original_call = subprocess.call
        self.launched = []

        def driver_call(command, **kwargs):
            # Use the current interpreter instead of platform-specific shebangs
            # or executable permission bits; the driver's process remains real.
            self.launched.append(command)
            if launch_error and command[command.index("--label") + 1] == "after":
                raise OSError("driver executable unavailable")
            return original_call([sys.executable, *command], **kwargs)

        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(sys, "argv", argv), \
                patch.object(editing_compare.os, "getloadavg", return_value=(1, 1, 1), create=True), \
                patch.object(editing_compare.subprocess, "run", return_value=SimpleNamespace(stdout="test uptime")), \
                patch.object(editing_compare.subprocess, "call", side_effect=driver_call), \
                contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = editing_compare.main()
        return code, stdout.getvalue(), stderr.getvalue()

    def test_fresh_result_and_forwarded_byte_offset(self):
        code, _, errors = self.invoke()
        self.assertEqual(code, 0, errors)
        result = json.loads(self.fresh.read_text())
        self.assertEqual(result["api_insertion_byte"], 17)
        self.assertIn("load_after", result)
        self.assertEqual(list(self.out.glob("*.pending")), [])

    def test_nonzero_exit_archives_previous_json_and_log(self):
        self.seed_previous()
        code, _, errors = self.invoke("exit")
        self.assertEqual(code, 1, errors)
        self.assertFalse(self.fresh.exists())
        archive, = self.out.glob("previous-*")
        self.assertEqual((archive / self.fresh.name).read_text(), self.old)
        self.assertEqual((archive / self.log.name).read_text(), "old log")
        self.assertIn("driver failure", self.log.read_text())

    def test_missing_output_cannot_reuse_previous_success(self):
        self.seed_previous()
        code, _, errors = self.invoke("missing")
        self.assertEqual(code, 1, errors)
        self.assertFalse(self.fresh.exists())
        self.assertIn("invalid driver output", errors)

    def test_invalid_json_is_preserved_for_inspection(self):
        code, _, errors = self.invoke("invalid")
        self.assertEqual(code, 1, errors)
        self.assertEqual([p.read_text() for p in self.out.glob("*.pending")], ["{broken"])
        self.assertFalse(self.fresh.exists())

    def test_invalid_shape_cannot_be_published(self):
        code, _, errors = self.invoke("shape")
        self.assertEqual(code, 1, errors)
        self.assertFalse(self.fresh.exists())

    def test_successful_rerun_preserves_old_artifacts(self):
        self.seed_previous()
        code, _, errors = self.invoke()
        self.assertEqual(code, 0, errors)
        archive, = self.out.glob("previous-*")
        self.assertEqual((archive / self.fresh.name).read_text(), self.old)
        self.assertNotIn("old", json.loads(self.fresh.read_text()))

    def test_launch_failure_continues_the_remaining_cohort(self):
        code, _, errors = self.invoke(options=[], launch_error=True)
        self.assertEqual(code, 1, errors)
        self.assertEqual(len(self.launched), 2)
        self.assertTrue((self.out / "sample.dart.0.json").exists())
        self.assertIn("driver executable unavailable", self.log.read_text())

    def test_report_only_requires_the_requested_cohort(self):
        self.seed_previous()
        code, _, errors = self.invoke(options=["--report-only"])
        self.assertEqual(code, 1, errors)
        self.assertIn("incomplete cohort", errors)
        self.assertEqual(self.launched, [])

    def test_report_only_rejects_invalid_saved_output(self):
        for contents in ("{broken", "{}"):
            with self.subTest(contents=contents):
                self.fresh.write_text(contents, encoding="utf-8")
                code, _, errors = self.invoke(options=["--report-only", "--only", "after"])
                self.assertEqual(code, 1, errors)
                self.assertIn("invalid saved output", errors)
                self.assertEqual(self.launched, [])

    def test_explicit_partial_report_remains_available(self):
        self.seed_previous()
        code, _, errors = self.invoke(options=["--report-only", "--only", "after"])
        self.assertEqual(code, 0, errors)
        self.assertEqual(self.launched, [])


if __name__ == "__main__":
    unittest.main()
