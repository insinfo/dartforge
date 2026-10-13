"""Runs the AOT ARC/tracing matrix with negative controls and raw artifacts.

A compilation failure is not accepted as a negative result. The controls must
end by a trap; a load error, a runtime abort or an unexpected output fails
the proof. The stale-layout control requires the specific abort of the
physical guard and its message, since the ABI does not allow unwinding the
panic into Dart. The manifest keeps commands, versions and file hashes.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile


PROOFS = {
    "arc_list_literal": ("proof-list-literal", False, [
        ("normal", None), ("normal", "no-owner"), ("normal", "no-release"),
    ]),
    "arc_list_storage": ("proof-list-storage", False, [
        ("normal", None), ("normal", "no-retain"), ("normal", "no-release"),
        ("--grow", None), ("--grow", "no-retain"), ("--grow", "no-release"),
    ]),
    "arc_simd_boxes": ("proof-simd-boxes", False, [("normal", None), ("normal", "no-release")]),
    "arc_typed_scalar_return": ("proof-typed-scalar-return", False, [
        (kind, control) for kind in ("--int", "--double", "--bool")
        for control in (None, "no-context-release")
    ]),
    "arc_native_port_owned": ("proof-native-port-owned", True, [
        (mode, control) for mode in ("check", "tables")
        for control in (None, "no-registration", "no-close")
    ]),
    "arc_source_return": ("proof-source-return", False, [
        ("normal", None), ("normal", "no-retain"), ("normal", "no-return-drop"),
        ("normal", "no-intermediate-drop"),
    ]),
    "arc_zeroed_instance": ("proof-zeroed-instance", False, [
        ("normal", None), ("normal", "no-owner"),
        ("normal", "no-table"), ("normal", "no-drop"),
        ("normal", "stale-layout"),
    ]),
    "arc_local_fields": ("proof-local-fields", False, [
        ("normal", None), ("normal", "no-retain"),
        ("normal", "no-object-drop"), ("normal", "no-return-drop"),
    ]),
    "arc_field_allocation": ("proof-field-allocation", False, [
        ("normal", None), ("normal", "no-child-drop"), ("normal", "no-object-owner"),
        ("--frame", None), ("--frame", "no-child-drop"), ("--frame", "no-object-owner"),
        ("--frame", "no-read-retain"),
        ("--typed", None), ("--typed", "no-child-drop"), ("--typed", "no-object-owner"),
        ("--typed", "no-context-release"),
        ("--typed-error", None), ("--typed-error", "no-context-release"),
    ]),
    "arc_field_keepalive": ("proof-field-keepalive", False, [
        ("normal", None), ("normal", "no-retain"), ("normal", "no-release"),
        ("--neighbor", None), ("--neighbor", "no-retain"), ("--neighbor", "no-release"),
        ("--argument", None), ("--argument", "no-retain"), ("--argument", "no-release"),
        ("--capture-neighbor", None), ("--capture-neighbor", "no-retain"), ("--capture-neighbor", "no-release"),
        ("--nullable", None), ("--nullable", "no-retain"), ("--nullable", "no-release"),
        ("--mixed", None), ("--mixed", "no-retain"), ("--mixed", "no-release"),
        ("--cycle", None), ("--cycle", "no-retain"), ("--cycle", "no-release"),
        ("--cell", None), ("--cell", "no-retain"), ("--cell", "no-release"),
        ("--context", None), ("--context", "no-retain"), ("--context", "no-release"),
        ("--global", None), ("--global", "no-retain"), ("--global", "no-release"),
        ("--late", None), ("--late", "no-retain"), ("--late", "no-release"),
    ]),
    "arc_keepalive_pending": ("proof-keepalive-pending", True, [
        ("normal", None), ("error", None), ("normal", "no-retain"),
        ("normal", "no-release"), ("error", "copy-on-error"),
    ]),
    "arc_loop_cleanup": ("proof-loop-cleanup", True, [
        ("normal", None), ("error", None), ("normal", "no-normal-drop"),
        ("error", "no-error-drop"),
    ]),
}


def info(command):
    return subprocess.check_output(command, text=True, encoding="utf-8").strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("example", choices=PROOFS)
    parser.add_argument("--debug", action="store_true", help="compile the generator without --release")
    parser.add_argument("--features", help="generator features, such as embedded-llvm")
    args = parser.parse_args()
    directory, has_path, cases = PROOFS[args.example]
    root = Path(__file__).resolve().parent.parent
    destination = root / "target" / directory
    destination.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update({
        "DARTFORGE_ARC_CHECK": "1",
        "DARTFORGE_ARC_NURSERY": "0",
        "DARTFORGE_GC_STRESS": "1",
    })
    # Intentional traps do not need to produce core dumps on the Unix runner.
    if os.name != "nt":
        import resource
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    manifest = {
        "source": info(["git", "-C", str(root), "rev-parse", "HEAD"]),
        "example": args.example,
        "example_sha256": hashlib.sha256((root / "crates/emit_native/examples" / (args.example + ".rs")).read_bytes()).hexdigest(),
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "source_git_status": info(["git", "-C", str(root), "status", "--porcelain", "--",
            "crates/emit_native", "crates/runtime", "scripts/prove-arc-aot.py"]),
        "rustc": info(["rustc", "--version"]),
        "environment": {k: environment[k] for k in (
            "DARTFORGE_ARC_CHECK", "DARTFORGE_ARC_NURSERY", "DARTFORGE_GC_STRESS")},
        "runs": [],
    }
    manifest_path = destination / "evidence.json"
    if args.example == "arc_source_return":
        sdk_lib = environment.get("DARTFORGE_SDK_LIB")
        if not sdk_lib:
            raise RuntimeError("the source proof requires DARTFORGE_SDK_LIB")
        manifest["sdk_lib"] = str(Path(sdk_lib).resolve())
        manifest["sdk_version"] = (Path(sdk_lib).parent / "version").read_text(encoding="utf-8").strip()
    target = (root / "target").resolve()
    if not target.is_relative_to(root):
        raise RuntimeError("temporary directory outside the repository")
    temporary = tempfile.TemporaryDirectory(prefix="tmp-arc-aot-", dir=target)
    environment["TEMP"] = environment["TMP"] = temporary.name
    try:
        for level in (0, 2):
            for mode in ("arc", "tracing"):
                for path, control in cases:
                    stem = f"{mode}-O{level}-{path}"
                    if control:
                        stem += "-" + control
                    exe = destination / (stem + (".exe" if os.name == "nt" else ""))
                    command = ["cargo", "run", "--locked"]
                    if not args.debug:
                        command.append("--release")
                    command += ["-p", "dartforge-emit-native"]
                    if args.features:
                        command += ["--features", args.features]
                    command += ["--example", args.example, "--", str(exe), f"--memory={mode}", str(level)]
                    if has_path or path.startswith("--"):
                        command.append(path)
                    if control:
                        command.append(control)
                    record = {"mode": mode, "level": level, "path": path,
                        "control": control, "generation": command, "files": []}
                    manifest["runs"].append(record)
                    log = destination / f"{stem}.build.log"
                    with log.open("wb") as out:
                        generation = subprocess.run(command, cwd=root, env=environment, stdout=out, stderr=subprocess.STDOUT)
                    record["generation_exit"] = generation.returncode
                    if generation.returncode:
                        raise RuntimeError(f"compilation failed: {stem}; see {log}")
                    stdout = destination / f"{stem}.stdout"
                    stderr = destination / f"{stem}.stderr"
                    with stdout.open("wb") as out, stderr.open("wb") as err:
                        process = subprocess.run([str(exe)], cwd=root, env=environment, stdout=out, stderr=err)
                    code = process.returncode
                    record["exit"] = code
                    (destination / f"{stem}.exit").write_bytes(f"{code}\n".encode("ascii"))
                    if args.example == "arc_zeroed_instance" and control == "stale-layout":
                        aborts = {0xC0000409, -1073740791} if os.name == "nt" else {-int(signal.SIGABRT)}
                        diagnostic = stderr.read_bytes()
                        if code not in aborts or stdout.read_bytes() != b"" or not all(
                            part in diagnostic for part in (
                                b"incompatible ARC scalar layout", b"class/size outside the Ref layouts")):
                            raise RuntimeError(f"the layout guard did not reject for the expected reason: {stem} ({code})")
                        record["rejection"] = "physical layout guard"
                    elif control:
                        traps = {0xC000001D, -1073741795} if os.name == "nt" else {-int(signal.SIGILL), -int(signal.SIGTRAP)}
                        if code not in traps or stdout.read_bytes() != b"":
                            raise RuntimeError(f"negative control did not end by a trap: {stem} ({code})")
                    elif code != 0 or stdout.read_bytes() != b"1\n":
                        raise RuntimeError(f"proof failed: {stem} ({code}); see {stdout} and {stderr}")
                    files = [log, exe.with_suffix(".ll"), stdout, stderr, destination / f"{stem}.exit"]
                    if args.example == "arc_source_return":
                        files.append(exe.with_suffix(".dart"))
                    for file in files:
                        data = file.read_bytes()
                        record["files"].append({"name": file.name, "bytes": len(data),
                            "sha256": hashlib.sha256(data).hexdigest()})
                    result = "layout guard" if record.get("rejection") else "trap" if control else "passed"
                    print(f"{args.example}: {stem}: {result}", flush=True)
        manifest["result"] = "success"
    except Exception as error:
        manifest["result"] = "failure"
        manifest["error"] = str(error)
        raise
    finally:
        try:
            # `open(newline=)` instead of `write_text(newline=)`, which requires Python 3.10.
            with manifest_path.open("w", encoding="utf-8", newline="\n") as output:
                output.write(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
        finally:
            temporary.cleanup()


if __name__ == "__main__":
    main()
