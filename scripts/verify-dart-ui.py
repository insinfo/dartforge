#!/usr/bin/env python3
"""Compare real dart_ui kernels on the VM, official AOT and DartForge AOT.

Uses the supplied framework checkout without modifying it or running pub get.
The default probe output contains correctness data, never benchmark timings.
"""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
from urllib.parse import urljoin


REPOSITORY = Path(__file__).resolve().parents[1]


def run(command, name, output, environment):
    result = subprocess.run(command, cwd=REPOSITORY, env=environment,
                            capture_output=True, timeout=300)
    (output / f"{name}.stdout").write_bytes(result.stdout)
    (output / f"{name}.stderr").write_bytes(result.stderr)
    if result.returncode:
        raise RuntimeError(f"{name} exited {result.returncode}; see {output / (name + '.stderr')}")
    return result.stdout


def configuration(framework, output):
    original = framework / ".dart_tool/package_config.json"
    config = json.loads(original.read_text())
    for package in config["packages"]:
        package["rootUri"] = urljoin(original.as_uri(), package["rootUri"])
    selected = [p for p in config["packages"] if p["name"] == "dart_ui"]
    if len(selected) != 1:
        raise RuntimeError("The package configuration must contain exactly one dart_ui package")
    selected[0]["rootUri"] = framework.as_uri() + "/"
    path = output / "package_config.json"
    path.write_text(json.dumps(config, indent=2) + "\n")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dart-ui-root", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=REPOSITORY / "target/dart-ui-compat")
    parser.add_argument("--dart", default="dart")
    parser.add_argument("--dartforge", type=Path, default=REPOSITORY / "target/p4/release/dartforge")
    parser.add_argument("--probe", action="append", choices=("video", "strip", "ffi"))
    parser.add_argument("--mode", action="append", choices=("tracing", "arc", "prepared"))
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(output), DARTFORGE_EFFECTS="check", DARTFORGE_ARC_CHECK="1")
    config = configuration(args.dart_ui_root.resolve(), output)
    branch = subprocess.run(["git", "branch", "--show-current"], cwd=args.dart_ui_root,
                            capture_output=True, text=True, check=True).stdout.strip()
    if branch != "main":
        raise RuntimeError("The framework checkout must be on main")
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=args.dart_ui_root,
                              capture_output=True, text=True, check=True).stdout.strip()
    framework_hash = hashlib.sha256()
    for source in sorted((args.dart_ui_root.resolve() / "lib").rglob("*.dart")):
        framework_hash.update(str(source.relative_to(args.dart_ui_root.resolve())).encode())
        framework_hash.update(b"\0")
        framework_hash.update(source.read_bytes())
        framework_hash.update(b"\0")
    metadata = {"framework_dart_sources_sha256": framework_hash.hexdigest(), "platform": platform.platform(), "architecture": platform.machine(), "dart_ui_revision": revision, "dart_ui_root": str(args.dart_ui_root.resolve()),
                "dart_sdk": subprocess.run([args.dart, "--version"], capture_output=True,
                                           text=True, check=True).stdout.strip(),
                "dartforge_sha256": hashlib.sha256(args.dartforge.resolve().read_bytes()).hexdigest(),
                "flags": ["--optimize", "--exceptions=tables", "--roots=shadow", "--stack-trace=none", "DARTFORGE_EFFECTS=check", "DARTFORGE_ARC_CHECK=1"],
                "performance_measurement": False, "results": []}
    for probe in args.probe or ["video", "strip", "ffi"]:
        source = REPOSITORY / f"bench/dart_ui/{probe}_probe.dart"
        source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
        reference = run([args.dart, f"--packages={config}", "run", str(source)], probe + "-vm", output, environment)
        official = output / f"{probe}-official"
        run([args.dart, "compile", "exe", f"--packages={config}", str(source), "-o", str(official)],
            probe + "-official-build", output, environment)
        if run([str(official)], probe + "-official", output, environment) != reference:
            raise RuntimeError(f"{probe}: official AOT differs from VM")
        for mode in args.mode or ["tracing", "arc", "prepared"]:
            native = output / f"{probe}-{mode}"
            current = dict(environment)
            current.pop("DARTFORGE_GC_STRESS", None)
            current.pop("DARTFORGE_GC_SCHEDULE", None)
            if mode == "prepared":
                current["DARTFORGE_ARC_PREPARE"] = "1"
            else:
                current.pop("DARTFORGE_ARC_PREPARE", None)
            run([str(args.dartforge.resolve()), "compile-native", str(source), "--packages", str(config),
                 "--optimize", "--exceptions=tables", "--roots=shadow", "--stack-trace=none", "--memory", "tracing" if mode == "tracing" else "arc",
                 "-o", str(native)], f"{probe}-{mode}-build", output, current)
            for stress in [False, True]:
                runtime = dict(current)
                if stress:
                    runtime["DARTFORGE_GC_STRESS"] = "1"
                name = f"{probe}-{mode}-{'stress' if stress else 'normal'}"
                actual = run([str(native)], name, output, runtime)
                if actual != reference:
                    raise RuntimeError(f"{name} differs from VM; compare {output / (name + '.stdout')}")
                metadata["results"].append({"probe": probe, "source_sha256": source_hash, "mode": mode,
                                            "gc_stress": stress, "stdout_bytes": len(actual), "matches_vm_and_official_aot": True})
                (output / "validation.json").write_text(json.dumps(metadata, indent=2) + "\n")
                print(f"{name}: exact match ({len(actual)} bytes)", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
