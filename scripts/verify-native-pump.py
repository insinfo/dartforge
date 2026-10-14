#!/usr/bin/env python3
"""Validate macOS native-pump behavior and record standalone idle CPU observations.

Uses resolved package dependencies without pub get. Official Dart references test
unsupported fallbacks; native capability assertions have their own expected output.
Idle observations are not paired benchmarks or evidence of a performance gain.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import shutil
import subprocess
import sys
import time
from urllib.parse import urljoin

REPOSITORY = Path(__file__).resolve().parents[1]
PROBES = {
    "main": ("native_pump_probe", "native pump main-queue microtask timer message validated\n"),
    "lifecycle": ("native_pump_lifecycle_probe", "native pump idle keepalive and callback shutdown validated\n"),
    "stress": ("native_pump_wake_stress_probe", "native pump wake stress messages=256 timers=256 native=32 validated\n"),
    "dispatcher": ("native_pump_dispatcher_probe", "native dispatcher outer-turn replacement disable roots validated\n"),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, name, output, environment, timeout=300, expected_exit=0):
    try:
        result = subprocess.run(command, cwd=REPOSITORY, env=environment,
                                capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        (output / f"{name}.stdout").write_bytes(error.stdout or b"")
        (output / f"{name}.stderr").write_bytes(error.stderr or b"")
        raise RuntimeError(f"{name} timed out after {timeout}s; logs saved in {output}") from error
    (output / f"{name}.stdout").write_bytes(result.stdout)
    (output / f"{name}.stderr").write_bytes(result.stderr)
    if result.returncode != expected_exit:
        raise RuntimeError(f"{name} exited {result.returncode}, expected {expected_exit}; see {output / (name + '.stderr')}")
    return result.stdout



def check_unhandled(command, name, output, environment):
    # VM baseline is measured first; runtime/core.rs has the same established
    # unhandled-exception exit policy. A signal/abort or any other exit fails.
    stdout = run(command, name, output, environment, timeout=30, expected_exit=255)
    stderr = (output / f"{name}.stderr").read_bytes()
    if (stdout or b"Unhandled exception:" not in stderr
            or b"Bad state: native dispatcher exception sentinel" not in stderr):
        raise RuntimeError(f"{name}: exception was hidden or incorrectly reported")
    return {"exit_code": 255, "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
            "unhandled_exception_visible": True, "sentinel_visible": True,
            "watchdog_seconds": 30}


def configuration(package_root, original, output):
    config = json.loads(original.read_text())
    for package in config["packages"]:
        package["rootUri"] = urljoin(original.as_uri(), package["rootUri"])
    selected = [p for p in config["packages"] if p["name"] == "dartforge"]
    if len(selected) != 1:
        raise RuntimeError("Configuration must resolve exactly one dartforge package")
    selected[0]["rootUri"] = package_root.as_uri() + "/"
    path = output / "package_config.json"
    path.write_text(json.dumps(config, indent=2) + "\n")
    return path


def cpu_seconds(pid):
    value = subprocess.run(["ps", "-p", str(pid), "-o", "time="],
                           capture_output=True, text=True, check=True).stdout.strip()
    if not value:
        raise RuntimeError("Idle process exited before its CPU reading")
    days = 0
    if "-" in value:
        prefix, value = value.split("-", 1)
        days = int(prefix)
    pieces = [float(part) for part in value.split(":")]
    total = 0.0
    for piece in pieces:
        total = total * 60 + piece
    return days * 86400 + total


def observe_idle(binary, mode, duration, output, environment):
    name = f"idle-{mode}"
    # Two additional seconds leave room for startup/CPU snapshots; the measured
    # interval starts only after the fixture reports that the native wait is ready.
    child = subprocess.Popen([str(binary), str(duration + 2)], cwd=REPOSITORY,
                             env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    ready = b""
    tail = b""
    error = b""
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            if not selector.select(timeout=30):
                raise RuntimeError("Idle fixture did not report readiness")
            ready = child.stdout.readline()
        if ready != b"native pump idle ready\n":
            raise RuntimeError(f"Unexpected idle readiness: {ready!r}")
        start_cpu = cpu_seconds(child.pid)
        start_wall = time.monotonic()
        time.sleep(duration)
        elapsed = time.monotonic() - start_wall
        end_cpu = cpu_seconds(child.pid)
        tail, error = child.communicate(timeout=30)
        if child.returncode or tail != b"native pump idle complete\n":
            raise RuntimeError(f"Idle fixture failed: exit {child.returncode}, {tail!r}, {error[-1000:]!r}")
        cpu = end_cpu - start_cpu
        if cpu < 0:
            raise RuntimeError("Process CPU time moved backwards")
        return {"mode": mode, "gc_stress": False, "requested_wall_seconds": duration,
                "measured_wall_seconds": elapsed, "process_cpu_seconds": cpu,
                "cpu_percent_of_one_core": cpu / elapsed * 100,
                "cpu_clock": "macOS ps TIME, approximately 0.01 second resolution",
                "paired_benchmark": False, "performance_improvement_claim": False}
    finally:
        if child.poll() is None:
            child.kill()
        tail, error = child.communicate()
        (output / f"{name}.stdout").write_bytes(ready + tail)
        (output / f"{name}.stderr").write_bytes(error)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=REPOSITORY / "target/native-pump-validation")
    parser.add_argument("--package-root", type=Path, default=REPOSITORY / "packages/dartforge")
    parser.add_argument("--packages", type=Path, help="Existing resolved package configuration")
    parser.add_argument("--dart", default="dart")
    parser.add_argument("--dartforge", type=Path, default=REPOSITORY / "target/p4/release/dartforge")
    parser.add_argument("--probe", action="append", choices=PROBES)
    parser.add_argument("--mode", action="append", choices=("tracing", "arc", "prepared"))
    parser.add_argument("--idle-seconds", type=int, default=3, choices=range(1, 29))
    parser.add_argument("--skip-idle", action="store_true")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise RuntimeError("Native CFRunLoop proof requires a macOS host")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    package_root = args.package_root.resolve()
    original = (args.packages or package_root / ".dart_tool/package_config.json").resolve()
    config = configuration(package_root, original, output)
    cli = args.dartforge.resolve()
    sdk = Path(shutil.which(args.dart) or args.dart).resolve()
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS="check", DARTFORGE_ARC_CHECK="1")
    for key in ("DARTFORGE_GC_STRESS", "DARTFORGE_GC_SCHEDULE", "DARTFORGE_ARC_PREPARE"):
        environment.pop(key, None)
    flags = ["--optimize", "--exceptions=tables", "--roots=shadow", "--stack-trace=none"]
    metadata = {"platform": platform.platform(), "architecture": platform.machine(),
                "dart_sdk": run([args.dart, "--version"], "sdk-version", output, environment).decode().strip(),
                "dart_executable": str(sdk), "dart_executable_sha256": digest(sdk),
                "dartforge": str(cli), "dartforge_sha256": digest(cli),
                "package_root": str(package_root), "original_package_config": str(original),
                "original_package_config_sha256": digest(original), "normalized_package_config_sha256": digest(config),
                "package_sources": {str(p.relative_to(package_root)): digest(p) for p in sorted((package_root / "lib").rglob("*.dart"))},
                "flags": flags, "validation_environment": {k: environment[k] for k in ("DARTFORGE_EFFECTS", "DARTFORGE_ARC_CHECK")},
                "reference_comparison": "VM and official AOT validate unsupported fallbacks; native validates capability-specific output",
                "contract_policy": "verify", "contract_policy_selection": "CLI default; no policy override",
                "paired_performance_measurement": False, "results": [], "idle_observations": [], "error_references": [], "expected_failures": []}

    def save():
        (output / "validation.json").write_text(json.dumps(metadata, indent=2) + "\n")

    save()
    modes = args.mode or ["tracing", "arc", "prepared"]
    probes = args.probe or list(PROBES)
    for probe in probes:
        stem, expected = PROBES[probe]
        source = package_root / "test" / f"{stem}.dart"
        reference = run([args.dart, f"--packages={config}", str(source)], probe + "-vm", output, environment)
        official = output / f"{probe}-official"
        run([args.dart, "compile", "exe", f"--packages={config}", str(source), "-o", str(official)],
            probe + "-official-build", output, environment)
        if run([str(official)], probe + "-official", output, environment) != reference:
            raise RuntimeError(f"{probe}: official AOT fallback differs from VM")
        if probe == "dispatcher":
            vm_error = check_unhandled([args.dart, f"--packages={config}", str(source), "--microtask-error"],
                                       "dispatcher-vm-error", output, environment)
            official_error = check_unhandled([str(official), "--microtask-error"],
                                             "dispatcher-official-error", output, environment)
            metadata["error_references"].append({"probe": probe, "vm": vm_error, "official_aot": official_error})
            save()
        for mode in modes:
            current = dict(environment)
            if mode == "prepared":
                current["DARTFORGE_ARC_PREPARE"] = "1"
            # Dotted variants deliberately exercise full-filename sidecars:
            # probe.arc and probe.tracing must not share probe.contracts.json.
            native = output / f"{probe}.{mode}"
            memory = "tracing" if mode == "tracing" else "arc"
            command = [str(cli), "compile-native", str(source), "--packages", str(config),
                       *flags, "--memory", memory, "-o", str(native)]
            run(command, f"{probe}-{mode}-build", output, current)
            manifest_path = Path(str(native) + ".contracts.json")
            manifest = json.loads(manifest_path.read_text())
            if (manifest.get("mode") != "verify" or manifest.get("certified") is not False
                    or manifest.get("verification_implemented") is not False):
                raise RuntimeError(f"{probe}-{mode}: unexpected contract policy/certification")
            artifact = manifest.get("artifact", {})
            if Path(artifact.get("path", "")).resolve() != native:
                raise RuntimeError(f"{probe}-{mode}: contract manifest refers to another artifact")
            provenance = {"path": str(manifest_path), "sha256": digest(manifest_path),
                          "mode": manifest["mode"], "certified": manifest["certified"],
                          "verification_implemented": manifest["verification_implemented"],
                          "artifact_blake3": artifact.get("blake3"),
                          "ir_blake3": artifact.get("ir_blake3")}
            for stress in (False, True):
                runtime = dict(current)
                if stress:
                    runtime["DARTFORGE_GC_STRESS"] = "1"
                name = f"{probe}-{mode}-{'stress' if stress else 'normal'}"
                actual = run([str(native)], name, output, runtime)
                if actual != expected.encode():
                    raise RuntimeError(f"{name}: unexpected capability output; see {output / (name + '.stdout')}")
                metadata["results"].append({"probe": probe, "source_sha256": digest(source), "mode": mode,
                                            "memory": memory, "arc_prepare": mode == "prepared", "gc_stress": stress,
                                            "compile_command": command, "binary_sha256": digest(native), "contract_manifest": provenance,
                                            "stdout_sha256": hashlib.sha256(actual).hexdigest(), "passed": True,
                                            "vm_and_official_fallback_sha256": hashlib.sha256(reference).hexdigest()})
                save()
                print(f"{name}: passed", flush=True)
                if probe == "dispatcher":
                    baseline = check_unhandled([str(native), "--microtask-error"], name + "-microtask-error", output, runtime)
                    failure = check_unhandled([str(native), "--throw"], name + "-dispatcher-error", output, runtime)
                    if failure["exit_code"] != baseline["exit_code"]:
                        raise RuntimeError(f"{name}: dispatcher changed the unhandled-exception policy")
                    metadata["expected_failures"].append({"probe": probe, "mode": mode, "gc_stress": stress,
                                                         "source_sha256": digest(source), "contract_manifest": provenance,
                                                         "ordinary_microtask": baseline, "native_dispatcher": failure,
                                                         "matches_existing_unhandled_policy": True})
                    save()
                    print(f"{name}: dispatcher exception matches microtask exit 255", flush=True)
            if probe == "stress" and not args.skip_idle:
                observation = observe_idle(native, mode, args.idle_seconds, output, current)
                metadata["idle_observations"].append(observation)
                save()
                print(f"idle-{mode}: {observation['process_cpu_seconds']:.2f}s CPU over {observation['measured_wall_seconds']:.3f}s wall (unpaired)", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
