"""Run the actual PowerShell shard selector against artifact arrival races."""
from pathlib import Path
import json
import subprocess
import tempfile
import unittest


SELECTOR = Path(__file__).with_name("native-scoreboard-inputs.ps1").resolve()


class NativeScoreboardInputsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.runner = self.root / "run.ps1"
        self.runner.write_text("""param($Selector, $Root, [int]$Expected)
$ErrorActionPreference = 'Stop'
$reports = @(& $Selector -Root $Root -ExpectedShards $Expected)
ConvertTo-Json -Compress -InputObject @($reports | ForEach-Object { $_.Directory.Name })
""", encoding="utf-8")

    def report(self, name, text="DartForge native: 1/1 ok\n"):
        path = self.root / name / "report.txt"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def select(self, expected=2):
        return subprocess.run(
            ["pwsh", "-NoProfile", "-File", str(self.runner),
             "-Selector", str(SELECTOR), "-Root", str(self.root),
             "-Expected", str(expected)], capture_output=True, text=True)

    def test_other_native_artifacts_never_change_the_shard_set(self):
        self.report("report-native-1")
        self.report("report-native-2")
        for name in ["arc", "arc-gc-stress", "io-windows", "sdk", "gc-stress", "A1", "B0", "B1"]:
            with self.subTest(name=name):
                self.report("report-native-" + name, "DartForge native: 0/999 ok\n")
                result = self.select()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout), ["report-native-1", "report-native-2"])

    def test_missing_and_duplicate_ids_fail_even_with_two_reports(self):
        self.report("report-native-1")
        self.report("report-native-arc")
        self.assertNotEqual(self.select().returncode, 0)
        self.report("nested/report-native-1")
        self.assertNotEqual(self.select().returncode, 0)

    def test_extra_numbered_shard_fails(self):
        for i in [1, 2, 3]:
            self.report(f"report-native-{i}")
        self.assertNotEqual(self.select().returncode, 0)

    def test_each_shard_requires_one_nonempty_consistent_scoreboard(self):
        self.report("report-native-1")
        for text in ["", "not a scoreboard\n", "DartForge native: 0/0 ok\n",
                     "DartForge native: 2/1 ok\n", "DartForge native: 1/1 ok\n" * 2]:
            with self.subTest(text=text):
                self.report("report-native-2", text)
                self.assertNotEqual(self.select().returncode, 0)

    def test_partial_score_is_preserved_for_the_existing_failure_gate(self):
        self.report("report-native-1", "DartForge native: 0/1 ok\n")
        self.report("report-native-2")
        result = self.select()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_numeric_order_handles_more_than_nine_shards(self):
        for i in range(1, 13):
            self.report(f"report-native-{i}")
        result = self.select(12)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(json.loads(result.stdout)), 12)


if __name__ == "__main__":
    unittest.main()
