#!/usr/bin/env python3
"""Compare prebuilt HTTP servers by CPU for a fixed number of keep-alive requests.

Uses bench/comparison/server.dart's routes and startup/shutdown protocol. The
Python client runs locally: wall throughput is client-limited, not server capacity.
On macOS, --sample saves one native CPU profile of the first server.
"""

import argparse
import http.client
import json
import os
from pathlib import Path
import platform
import re
import selectors
import statistics
import subprocess
import time


def process_cpu(pid):
    if platform.system() == "Linux":
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")
    raw = subprocess.check_output(["ps", "-o", "cputime=", "-p", str(pid)], text=True).strip()
    parts = raw.split("-")
    days = int(parts[0]) if len(parts) == 2 else 0
    seconds = 0.0
    for part in parts[-1].split(":"):
        seconds = seconds * 60 + float(part)
    return days * 86400 + seconds


def requests(connection, route, count, expected_id, request_body):
    for _ in range(count):
        connection.request("POST" if request_body else "GET", route, body=request_body,
                           headers={"Content-Type": "application/octet-stream"} if request_body else {})
        response = connection.getresponse()
        response_body = response.read()
        if response.status != 200:
            raise RuntimeError(f"HTTP {response.status}: {response_body!r}")
        if route in ("/json", "/json-utf8"):
            expected_id += 1
            expected = {"message": "Hello, World!", "id": expected_id,
                        "active": True, "items": [1, 2, 3]}
            if response_body != json.dumps(expected, separators=(",", ":")).encode():
                raise RuntimeError(f"Unexpected JSON: {response_body!r}")
        elif response_body != b"hello":
            raise RuntimeError(f"Unexpected text response: {response_body!r}")
    return expected_id


def measure(executable, route, count, warmup, sample_path, request_bytes):
    server = subprocess.Popen([str(executable), "0"], stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True)
    connection = None
    sampler = None
    try:
        with selectors.DefaultSelector() as ready:
            ready.register(server.stdout, selectors.EVENT_READ)
            if not ready.select(timeout=15):
                raise TimeoutError("Server did not report its port within 15 seconds")
        line = server.stdout.readline().strip()
        match = re.fullmatch(r"port (\d+)", line)
        if not match:
            raise RuntimeError(f"Server did not report its port: {line!r}")
        connection = http.client.HTTPConnection("127.0.0.1", int(match[1]), timeout=15)
        body = b"x" * request_bytes if request_bytes else None
        expected_id = requests(connection, route, warmup, 0, body)
        if sample_path:
            sampler = subprocess.Popen(["sample", str(server.pid), "5", "1", "-file",
                                        str(sample_path)], stdout=subprocess.DEVNULL,
                                       stderr=subprocess.DEVNULL)
        cpu_start = process_cpu(server.pid)
        start = time.monotonic()
        requests(connection, route, count, expected_id, body)
        elapsed = time.monotonic() - start
        cpu = process_cpu(server.pid) - cpu_start
        rss = int(subprocess.check_output(["ps", "-o", "rss=", "-p", str(server.pid)], text=True))
        if sampler:
            sampler.wait(timeout=15)
            if sampler.returncode != 0:
                raise RuntimeError("sample failed to collect the CPU profile")
        connection.request("GET", "/quit")
        response = connection.getresponse()
        if response.read() != b"bye":
            raise RuntimeError("Unexpected shutdown response")
        connection.close()
        server.wait(timeout=15)
        if server.returncode != 0:
            raise RuntimeError(server.stderr.read())
        return {"requests": count, "request_body_bytes": request_bytes,
                "cpu_seconds": cpu, "elapsed_seconds": elapsed,
                "cpu_ms_per_1000_requests": cpu * 1e6 / count,
                "client_limited_requests_per_second": count / elapsed, "rss_kib": rss}
    finally:
        if connection:
            connection.close()
        if sampler and sampler.poll() is None:
            sampler.kill()
            sampler.wait()
        if server.poll() is None:
            server.kill()
            server.wait()
        server.stdout.close()
        server.stderr.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executables", nargs="+", type=Path)
    parser.add_argument("--requests", type=int, default=50000)
    parser.add_argument("--warmup", type=int, default=1000)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--request-bytes", type=int, default=0,
                        help="POST this many body bytes instead of GET; the benchmark server drains the body")
    parser.add_argument("--routes", default="/,/json",
                        help="comma-separated routes: /, /json, /json-utf8")
    parser.add_argument("--sample", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.requests <= 0 or args.warmup < 0 or args.rounds <= 0 or args.request_bytes < 0:
        parser.error("request and round counts must be positive; warmup and body size must be nonnegative")
    if args.sample and platform.system() != "Darwin":
        parser.error("--sample requires macOS")
    executables = [p.resolve(strict=True) for p in args.executables]
    routes = args.routes.split(",")
    if any(route not in ("/", "/json", "/json-utf8") for route in routes):
        parser.error("unsupported benchmark route")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.sample:
        args.sample.parent.mkdir(parents=True, exist_ok=True)
    results = []
    for round_number in range(args.rounds):
        order = executables if round_number % 2 == 0 else list(reversed(executables))
        for executable in order:
            for route in routes:
                sample = args.sample if round_number == 0 and executable == executables[0] and route == "/json" else None
                result = measure(executable, route, args.requests, args.warmup, sample, args.request_bytes)
                result.update(executable=str(executable), route=route, round=round_number + 1)
                results.append(result)
                print(f"{executable.name} {route} round={round_number + 1}: "
                      f"CPU {result['cpu_ms_per_1000_requests']:.3f} ms/1000 requests", flush=True)
    medians = []
    for executable in executables:
        for route in routes:
            values = [r["cpu_ms_per_1000_requests"] for r in results
                      if r["executable"] == str(executable) and r["route"] == route]
            medians.append({"executable": str(executable), "route": route,
                            "median_cpu_ms_per_1000_requests": statistics.median(values),
                            "minimum": min(values), "maximum": max(values)})
    args.output.write_text(json.dumps({"platform": platform.platform(),
        "limitations": ["Local Python client limits wall throughput", "One keep-alive connection",
                        "macOS CPU counter resolution is 0.01 seconds"],
        "results": results, "medians": medians}, indent=2) + "\n")


if __name__ == "__main__":
    main()
