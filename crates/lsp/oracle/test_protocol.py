"""Deterministic protocol deadline and readiness tests; no installed SDK needed."""

import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import oracle


class Clock:
    def __init__(self):
        self.now = 0.0
        self.events = []

    def advance(self, seconds):
        end = self.now + seconds
        while self.events and self.events[0][0] <= end:
            self.now, callback = self.events.pop(0)
            callback()
        self.now = end


def peer(name, analyzing=None):
    server = SimpleNamespace(name=name, cv=threading.Condition(), analyzing=analyzing,
                             analysis_starts=0, analysis_completions=0)
    return server


def status(server, busy):
    server.analyzing = busy
    if busy:
        server.analysis_starts += 1
    else:
        server.analysis_completions += 1


class ProtocolTests(unittest.TestCase):
    def idle(self, clock, servers, **kwargs):
        with patch.object(oracle.time, "monotonic", lambda: clock.now), \
                patch.object(oracle.time, "sleep", clock.advance):
            return oracle.wait_all_idle(servers, **kwargs)

    def test_unknown_status_is_not_idle(self):
        clock = Clock()
        self.assertFalse(self.idle(clock, [peer("dart")], limit=0.1, quiet=0.01))
        self.assertAlmostEqual(clock.now, 0.1)

    def test_both_peers_must_be_idle_for_the_shared_interval(self):
        clock = Clock()
        dart, forge = peer("dart", True), peer("dartforge", True)
        clock.events = [(0.02, lambda: status(dart, False)),
                        (0.12, lambda: status(forge, False))]
        self.assertTrue(self.idle(clock, [dart, forge], limit=1, quiet=0.1))
        self.assertGreaterEqual(clock.now, 0.22)

    def test_busy_cycle_between_polls_resets_the_shared_interval(self):
        clock = Clock()
        server = peer("dart", False)
        clock.events = [(0.01, lambda: status(server, True)),
                        (0.02, lambda: status(server, False))]
        self.assertTrue(self.idle(clock, [server], limit=1, quiet=0.08))
        self.assertGreaterEqual(clock.now, 0.1)

    def test_quiet_wait_cannot_extend_the_deadline(self):
        clock = Clock()
        server = peer("dart", True)
        clock.events = [(0.09, lambda: status(server, False))]
        self.assertFalse(self.idle(clock, [server], limit=0.1, quiet=0.05))
        self.assertAlmostEqual(clock.now, 0.1)

    def test_empty_peer_list_cannot_prove_readiness(self):
        with self.assertRaises(ValueError):
            oracle.wait_all_idle([])

    def test_legacy_status_quiet_interval_also_obeys_its_deadline(self):
        clock = Clock()
        server = object.__new__(oracle.Lsp)
        server.analysis_completions = 1
        server.analyzing = False

        class Condition:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                pass

            def wait(self, timeout):
                clock.advance(timeout)
                return False

        server.cv = Condition()
        with patch.object(oracle.time, "monotonic", lambda: clock.now):
            self.assertFalse(server.wait_analysis(0, limit=0.1, quiet=1))
        self.assertAlmostEqual(clock.now, 0.1)

    def test_unrelated_notifications_do_not_restart_a_request_timeout(self):
        clock = Clock()
        server = object.__new__(oracle.Lsp)
        server.next_id = 0
        server.responses = {}
        sent = []
        server._send = sent.append

        class Notifications:
            def __init__(self):
                self.waits = 0

            def __enter__(self):
                return self

            def __exit__(self, *_):
                pass

            def wait(self, timeout):
                self.waits += 1
                if self.waits > 10:
                    raise AssertionError("request deadline restarted on notifications")
                clock.advance(min(0.03, timeout))
                return True

        server.cv = Notifications()
        with patch.object(oracle.time, "perf_counter", lambda: clock.now):
            reply, milliseconds = server.request("workspace/symbol", {"query": "x"}, limit=0.1)
        self.assertEqual(reply, {"error": "timed out"})
        self.assertAlmostEqual(milliseconds, 100)
        self.assertLessEqual(server.cv.waits, 4)
        self.assertEqual(sent[0]["params"], {"query": "x"})

    def test_observer_keeps_protocol_frames_and_status_results_unchanged(self):
        frames = [{"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": True}},
                  {"jsonrpc": "2.0", "method": "$/analyzerStatus", "params": {"isAnalyzing": False}},
                  {"jsonrpc": "2.0", "id": 1, "result": []}]
        wire = b""
        for frame in frames:
            body = json.dumps(frame).encode()
            wire += f"Content-Length: {len(body)}\r\n\r\n".encode() + body
        server = object.__new__(oracle.Lsp)
        server.p = SimpleNamespace(stdout=io.BytesIO(wire), stdin=io.BytesIO())
        server.cv = threading.Condition()
        server.responses = {}
        server.analyzing = None
        server.analysis_starts = server.analysis_completions = 0
        observations = []
        server.observer = lambda direction, frame: observations.append((direction, frame))
        server._read()
        self.assertEqual(server.responses[1], frames[2])
        self.assertEqual(server.analysis_starts, 1)
        self.assertEqual(server.analysis_completions, 1)
        self.assertIs(server.analyzing, False)
        self.assertEqual(observations, [("received", frame) for frame in frames])
        server._send(frames[2])
        body = server.p.stdin.getvalue().split(b"\r\n\r\n", 1)[1]
        self.assertEqual(json.loads(body), frames[2])
        self.assertEqual(observations[-1], ("sent", frames[2]))

    def test_settle_retries_only_transient_not_analyzed_and_prepares_both_peers(self):
        clock = Clock()
        dart, forge = peer("dart", True), peer("dartforge", True)
        requests = {"dart": [], "dartforge": []}

        def request(server):
            def call(method, params, limit):
                requests[server.name].append((method, params))
                clock.advance(0.01)
                if server is dart and len(requests[server.name]) == 1:
                    return {"error": {"code": -32007}}, 10
                return [], 10
            return call

        for server in (dart, forge):
            server.request = request(server)
        clock.events = [(0.11, lambda: status(dart, False)),
                        (0.21, lambda: status(forge, False))]
        with patch.object(oracle.time, "monotonic", lambda: clock.now), \
                patch.object(oracle.time, "sleep", clock.advance):
            document = Path(tempfile.gettempdir()).resolve() / "dartforge-protocol-entry.dart"
            result = oracle.settle_initial_analysis([dart, forge], [document], limit=1, quiet=0.1)
        self.assertEqual(requests["dart"], requests["dartforge"])
        self.assertEqual(len(requests["dart"]), 2)
        self.assertGreaterEqual(clock.now, 0.31)
        self.assertTrue(all(s["analyzing"] is False for s in result["states"].values()))

    def test_settle_does_not_hide_an_unexpected_resolution_error(self):
        server = peer("dart", False)
        server.request = lambda *args, **kwargs: ({"error": {"code": -32603}}, 0)
        with self.assertRaisesRegex(RuntimeError, "initial document resolution failed"):
            oracle.settle_initial_analysis([server], [Path(tempfile.gettempdir()).resolve() / "dartforge-protocol-entry.dart"])

    def test_readiness_metadata_does_not_inflate_feature_sample_counts(self):
        with tempfile.TemporaryDirectory(prefix="dartforge-readiness-summary-") as temporary:
            path = Path(temporary) / "rows.jsonl"
            path.write_text(json.dumps({"method": "oracle/initialReadiness", "mode": "settled", "states": {}})
                            + "\n" + json.dumps({"method": "workspace/symbol", "dart": [], "dartforge": []}) + "\n", encoding="utf-8")
            result = subprocess.run([sys.executable, str(Path(__file__).with_name("summary.py")), str(path)],
                                    check=True, text=True, capture_output=True)
        self.assertIn("| symbol: complete items (ordered) | 1 | 100%", result.stdout)
        self.assertNotIn("oracle/initialReadiness", result.stdout)


if __name__ == "__main__":
    unittest.main()
