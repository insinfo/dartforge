#!/usr/bin/env python3
"""Verify resident AOT callback tombstones, owner teardown and finalization order.

Official SDK references only execute the safe extension-skip branch. Native
cases deliberately invoke closed callbacks, which is undefined on the Dart VM.
The pthread helper and atexit assertions currently run on macOS/Linux hosts.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / "crates/runtime/tests/ffi_late_close_probe.dart"
FINALIZE = ROOT / "crates/runtime/tests/ffi_callback_finalize_probe.dart"
HELPER = ROOT / "crates/runtime/tests/ffi_late_close_helper.c"
SKIP = "Late-call extension requires DartForge AOT\n"
SUCCESS = "late callback exceptional returns, stable pointers and foreign drops verified\n"
ATEXIT = "atexit callback exceptional return verified\n"


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, default=ROOT / "target/p4/release/dartforge")
    parser.add_argument("--dart", default=shutil.which("dart"))
    parser.add_argument("--clang", default=os.environ.get("DARTFORGE_CLANG", shutil.which("clang")))
    parser.add_argument("--sdk", type=Path, default=Path("/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib"))
    parser.add_argument("--output", type=Path, default=ROOT / "target/ffi-late-close-validation")
    args = parser.parse_args()
    if sys.platform not in ("darwin", "linux"):
        parser.error("The native pthread/atexit helper requires macOS or Linux")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    cli = args.cli.resolve()
    sources = [Path(__file__).resolve(), PROBE, FINALIZE, HELPER]
    start = {str(path.relative_to(ROOT)): sha(path) for path in sources}
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS="check", DARTFORGE_ARC_CHECK="1", DARTFORGE_CLANG=args.clang)
    for key in ("DARTFORGE_KEEP_SYMBOLS", "DARTFORGE_GC_STRESS", "DARTFORGE_GC_SCHEDULE", "DARTFORGE_ARC_PREPARE", "DARTFORGE_ARC_PREPARED"):
        environment.pop(key, None)
    ledger = {"cli": str(cli), "cli_sha256": sha(cli), "host": platform.platform(), "source_hashes": start,
              "compile_environment": {key: environment[key] for key in ("DARTFORGE_EFFECTS", "DARTFORGE_ARC_CHECK", "DARTFORGE_CLANG")},
              "default_stripped_symbols": True, "jit_reload_certified": False, "results": []}

    def run(command, name, env=None, expected=0, timeout=300):
        result = subprocess.run([str(value) for value in command], cwd=ROOT,
                                env=environment if env is None else env,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
        (output / (name + ".stdout")).write_bytes(result.stdout)
        (output / (name + ".stderr")).write_bytes(result.stderr)
        if result.returncode != expected:
            raise RuntimeError(f"{name}: exit {result.returncode}, expected {expected}; see {output}")
        return result.stdout.decode(), result.stderr.decode()

    version, _ = run([args.dart, "--version"], "sdk-version")
    ledger["sdk_version"] = version.strip()
    if "3.6.2" not in version:
        raise RuntimeError("This oracle requires Dart SDK 3.6.2")
    clang_version, _ = run([args.clang, "--version"], "clang-version")
    ledger["clang_version"] = clang_version.strip()
    if "version 22." not in clang_version:
        raise RuntimeError("This oracle requires LLVM clang 22")
    helper = output / ("ffi_late_close_helper.dylib" if sys.platform == "darwin" else "ffi_late_close_helper.so")
    helper_command = [args.clang, "-Wall", "-Wextra", "-Werror"]
    if sys.platform == "darwin":
        sdk_path, _ = run(["xcrun", "--show-sdk-path"], "mac-sdk-path")
        helper_command += ["-isysroot", sdk_path.strip(), "-dynamiclib"]
        ledger["mac_sdk_path"] = sdk_path.strip()
    else:
        helper_command += ["-shared", "-fPIC", "-pthread"]
    helper_command += [str(HELPER), "-o", str(helper)]
    ledger["helper_build_command"] = helper_command
    run(helper_command, "helper-build")
    ledger["helper_sha256"] = sha(helper)
    for label, source in (("callbacks", PROBE), ("finalize", FINALIZE)):
        stdout, stderr = run([args.dart, source], label + "-vm")
        if stdout != SKIP or stderr:
            raise RuntimeError(f"{label}: official VM did not take the safe skip branch")
        official = output / (label + ".official")
        run([args.dart, "compile", "exe", source, "-o", official], label + "-official-build")
        stdout, stderr = run([official], label + "-official")
        if stdout != SKIP or stderr:
            raise RuntimeError(f"{label}: official AOT did not take the safe skip branch")
    for mode in ("tracing", "arc", "prepared"):
        compile_env = dict(environment)
        if mode == "prepared":
            compile_env["DARTFORGE_ARC_PREPARED"] = "1"
        for label, source in (("callbacks", PROBE), ("finalize", FINALIZE)):
            binary = output / (label + "." + mode)
            command = [cli, "aot", source, binary, "--optimize", "--memory", "tracing" if mode == "tracing" else "arc", "--sdk", args.sdk]
            run(command, label + "-" + mode + "-build", compile_env)
            manifest_path = Path(str(binary) + ".contracts.json")
            manifest = json.loads(manifest_path.read_text())
            if manifest["mode"] != "verify" or manifest["certified"] is not False or manifest["verification_implemented"] is not False:
                raise RuntimeError("Unexpected contract policy/certification")
            if Path(manifest["artifact"]["path"]).resolve() != binary:
                raise RuntimeError("Manifest belongs to another binary")
            artifact = {"binary_sha256": sha(binary), "manifest_path": str(manifest_path),
                        "manifest_sha256": sha(manifest_path), "manifest": manifest,
                        "compile_command": [str(value) for value in command],
                        "preparation_environment": {key: compile_env[key] for key in ("DARTFORGE_ARC_PREPARED",) if key in compile_env}}
            for stress in (False, True):
                runtime = dict(compile_env)
                if stress:
                    runtime["DARTFORGE_GC_STRESS"] = "1"
                for error in ((False, True) if label == "finalize" else (False,)):
                    name = f"{label}-{mode}-{'stress' if stress else 'normal'}" + ("-error" if error else "")
                    expected = 255 if error else 0
                    command = [binary, helper] + (["--error"] if error else [])
                    stdout, stderr = run(command, name, runtime, expected, timeout=30)
                    if label == "callbacks":
                        if stdout != SUCCESS or stderr:
                            raise RuntimeError(name + ": callback oracle output mismatch")
                    elif error:
                        if stdout or "Unhandled exception:" not in stderr or "error callback lifetime verified: 15" not in stderr or not stderr.endswith(ATEXIT):
                            raise RuntimeError(name + ": error formatting/finalization order mismatch")
                    elif stdout != "normal main completed before callback retirement\n" or stderr != ATEXIT:
                        raise RuntimeError(name + ": normal finalization order mismatch")
                    ledger["results"].append(dict(label=label, mode=mode, stress=stress, error=error,
                        expected_exit=expected, stdout=stdout, stderr=stderr, artifact=artifact))
                    print(name + " PASS", flush=True)
    if {str(path.relative_to(ROOT)): sha(path) for path in sources} != start or sha(cli) != ledger["cli_sha256"]:
        raise RuntimeError("Validation sources or compiler changed while running")
    (output / "validation.json").write_text(json.dumps(ledger, indent=2) + "\n")
    print(f"18 native cases verified; ledger: {output / 'validation.json'}", flush=True)


if __name__ == "__main__":
    main()
