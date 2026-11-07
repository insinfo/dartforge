#!/usr/bin/env python3
"""Production JS must satisfy a successful backend reference independently of VM failures."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("language_suite", Path(__file__).with_name("run-language-suite.py"))
suite = importlib.util.module_from_spec(spec)
spec.loader.exec_module(suite)


class ProductionBackendReference(unittest.TestCase):
    def result(self, code, output=""):
        return {"code": code, "out": output, "err": ""}

    def test_shared_runtime_failure_cannot_hide_a_dart2js_success(self):
        # The original two variance tests fail on the VM but pass on dart2js.
        failed = self.result(255)
        self.assertIsNone(suite.matches("jsprod", "variance", failed,
            {"variance": failed}, {"variance": self.result(0)}))

    def test_a_real_backend_success_can_pass_where_vm_expectation_fails(self):
        vm = {"code": 255, "out": "", "err": "Unhandled exception:\nExpect.type failed\n"}
        passed = self.result(0)
        self.assertEqual(suite.matches("jsprod", "variance", passed,
            {"variance": vm}, {"variance": passed}), "passes")

    def test_successful_vm_result_keeps_its_category(self):
        passed = self.result(0, "ok\n")
        self.assertEqual(suite.matches("jsprod", "ordinary", passed,
            {"ordinary": passed}, {"ordinary": passed}), "vm")

    def test_existing_shared_failure_is_retained_when_backend_also_fails(self):
        failed = self.result(255)
        self.assertEqual(suite.matches("jsprod", "shared", failed,
            {"shared": failed}, {"shared": failed}), "vm")

    def test_native_classification_does_not_use_the_js_backend_contract(self):
        failed = self.result(255)
        self.assertEqual(suite.matches("native", "native", failed,
            {"native": failed}, {"native": self.result(0)}), "vm")


if __name__ == "__main__":
    unittest.main()
