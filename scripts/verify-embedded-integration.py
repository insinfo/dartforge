#!/usr/bin/env python3
"""Verify finite AppKit dispatch without a window or nested Dart event turns.

Uses existing package configurations without pub get or framework writes. VM and
upstream AOT exercise an unsupported fallback; native traces have their own exact
oracle. Records distinct compiler contract sidecars for each memory-mode binary.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
from urllib.parse import urljoin

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = (b"failed launch preserved error and cleared delegate/sink\n"
            b"main-thread=true launched=true synthetic=1 message=23 timer=1\n"
            b"embedded integration shutdown verified; no window or modal loop\n")
FALLBACK = b"embedded integration unsupported: native pump/dispatcher unavailable\n"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(command, label, output, environment, timeout=180):
    try:
        result = subprocess.run(command, cwd=ROOT, env=environment,
                                capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        (output / (label + ".stdout")).write_bytes(error.stdout or b"")
        (output / (label + ".stderr")).write_bytes(error.stderr or b"")
        raise RuntimeError(f"{label} timed out; see logs in {output}") from error
    (output / (label + ".stdout")).write_bytes(result.stdout)
    (output / (label + ".stderr")).write_bytes(result.stderr)
    if result.returncode:
        raise RuntimeError(f"{label} exited {result.returncode}; see logs in {output}")
    return result


def package_configuration(framework, output):
    packages = {}
    for original in (framework / ".dart_tool/package_config.json",
                     ROOT / "packages/dartforge/.dart_tool/package_config.json"):
        data = json.loads(original.read_text())
        for package in data["packages"]:
            package = dict(package)
            package["rootUri"] = urljoin(original.as_uri(), package["rootUri"])
            packages[package["name"]] = package
    for name, directory in (("dart_ui", framework), ("dartforge", ROOT / "packages/dartforge")):
        if name not in packages:
            raise RuntimeError(f"Missing {name} in existing package configurations")
        packages[name]["rootUri"] = directory.as_uri() + "/"
    if "meta" not in packages:
        raise RuntimeError("Existing configurations must resolve package:meta")
    path = output / "package_config.json"
    path.write_text(json.dumps({"configVersion": 2, "packages": list(packages.values())}, indent=2) + "\n")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--framework", "--dart-ui-root", required=True, type=Path)
    parser.add_argument("--compiler", type=Path, default=ROOT / "target/p4/release/dartforge")
    parser.add_argument("--dart", default="dart")
    parser.add_argument("--output", type=Path, default=ROOT / "target/embedded-integration")
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("The live AppKit probe currently requires macOS arm64")
    framework, compiler, output = args.framework.resolve(), args.compiler.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    def git(*arguments):
        return subprocess.check_output(["git", "-C", str(framework), *arguments])
    if git("branch", "--show-current").strip() != b"main":
        raise RuntimeError("The framework checkout must be on main")
    config = package_configuration(framework, output)
    source = ROOT / "bench/dart_ui/embedded_integration_probe.dart"
    patch = git("diff", "HEAD", "--binary", "--", "lib")
    (output / "framework.patch").write_bytes(patch)
    sources = {str(path.relative_to(framework)): sha256(path)
               for path in sorted((framework / "lib").rglob("*.dart"))}
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS="check", DARTFORGE_ARC_CHECK="1")
    for name in ("DARTFORGE_GC_STRESS", "DARTFORGE_GC_SCHEDULE", "DARTFORGE_ARC_PREPARED", "DARTFORGE_ARC_PREPARE", "DARTFORGE_CONTRACTS"):
        environment.pop(name, None)
    metadata = {"compiler": str(compiler), "compiler_sha256": sha256(compiler),
                "source": str(source), "source_sha256": sha256(source),
                "runner_sha256": sha256(Path(__file__).resolve()),
                "package_config_sha256": sha256(config),
                "dart_sdk": subprocess.check_output([args.dart, "--version"], text=True).strip(),
                "framework": str(framework), "framework_head": git("rev-parse", "HEAD").decode().strip(),
                "framework_status": git("status", "--porcelain").decode(),
                "framework_dirty_patch_sha256": hashlib.sha256(patch).hexdigest(),
                "framework_dart_sources": sources, "fallbacks": {}, "builds": [], "runs": [],
                "expected_native_trace": EXPECTED.decode(), "performance_measurement": False,
                "limits": ["No window, rendering, user input, modal-loop progress or nested Dart event turns are tested.",
                           "Upstream fallbacks are verified separately, not compared to the positive native trace.",
                           "The injected Dart failure checks delegate/sink cleanup; no public port/refcount diagnostic is available.",
                           "Manifest artifact paths and stable binary SHA256 bind the ledger; compiler-provided BLAKE3 values are recorded without a third-party recomputation."]}
    def save():
        (output / "validation.json").write_text(json.dumps(metadata, indent=2) + "\n")
    save()
    reference = invoke([args.dart, f"--packages={config}", str(source)], "vm", output, environment)
    official = output / "official"
    invoke([args.dart, "compile", "exe", f"--packages={config}", str(source), "-o", str(official)], "official-build", output, environment)
    for name, result in (("vm", reference), ("official", invoke([str(official)], "official", output, environment))):
        if result.stdout != FALLBACK or result.stderr:
            raise RuntimeError(f"{name}: unexpected unsupported fallback")
        metadata["fallbacks"][name] = {"exit_code": 0, "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(), "unsupported_fallback": True}
    save()
    paths = set()
    for mode in ("tracing", "arc", "prepared"):
        current = dict(environment)
        if mode == "prepared":
            current["DARTFORGE_ARC_PREPARED"] = "1"
        binary = output / ("integration." + mode)
        command = [str(compiler), "compile-native", str(source), "--packages", str(config),
                   "--optimize", "--exceptions=tables", "--roots=shadow", "--stack-trace=none",
                   "--dartforge-contracts=verify", "--memory=" + ("tracing" if mode == "tracing" else "arc"), "-o", str(binary)]
        invoke(command, mode + "-build", output, current)
        manifest_path = Path(str(binary) + ".contracts.json")
        if manifest_path in paths:
            raise RuntimeError("Contract sidecar path collision")
        paths.add(manifest_path)
        manifest = json.loads(manifest_path.read_text())
        artifact = manifest["artifact"]
        if Path(artifact["path"]).resolve() != binary or manifest["mode"] != "verify":
            raise RuntimeError(f"{mode}: manifest is not bound to this artifact")
        if manifest.get("requests") or manifest.get("certified") or manifest.get("diagnostics"):
            raise RuntimeError(f"{mode}: this unannotated probe must not claim a contract certificate")
        binary_hash, manifest_hash = sha256(binary), sha256(manifest_path)
        metadata["builds"].append({"mode": mode, "command": command,
                                  "prepared": mode == "prepared", "artifact_sha256": binary_hash,
                                  "manifest_path": str(manifest_path), "manifest_sha256": manifest_hash,
                                  "artifact_identity": artifact, "manifest_bound_to_artifact_path": True})
        save()
        for stress in (False, True):
            runtime = dict(current)
            if stress:
                runtime["DARTFORGE_GC_STRESS"] = "1"
            label = mode + ("-stress" if stress else "-normal")
            result = invoke([str(binary)], label, output, runtime, timeout=45)
            if result.stdout != EXPECTED or result.stderr:
                raise RuntimeError(f"{label}: unexpected trace/stderr; see logs in {output}")
            if sha256(binary) != binary_hash or sha256(manifest_path) != manifest_hash:
                raise RuntimeError(f"{label}: artifact or sidecar changed after compilation")
            metadata["runs"].append({"mode": mode, "gc_stress": stress, "exit_code": 0,
                                     "environment": {key: value for key, value in runtime.items() if key.startswith("DARTFORGE_")},
                                     "stdout_bytes": len(result.stdout), "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                                     "stderr_bytes": 0, "matches_explicit_expected_trace": True,
                                     "artifact_sha256": binary_hash, "manifest_path": str(manifest_path),
                                     "manifest_sha256": manifest_hash})
            save()
            print(f"{label}: verified live launch, cleanup and dispatch", flush=True)
    if sha256(compiler) != metadata["compiler_sha256"] or sha256(source) != metadata["source_sha256"]:
        raise RuntimeError("Compiler or source changed during validation")
    final_sources = {str(path.relative_to(framework)): sha256(path)
                     for path in sorted((framework / "lib").rglob("*.dart"))}
    if final_sources != sources or git("diff", "HEAD", "--binary", "--", "lib") != patch:
        raise RuntimeError("Framework source or patch changed during validation")
    if git("rev-parse", "HEAD").decode().strip() != metadata["framework_head"]:
        raise RuntimeError("Framework revision changed during validation")
    metadata["inputs_stable_through_run"] = True
    save()
    print(output / "validation.json", flush=True)


if __name__ == "__main__":
    main()
