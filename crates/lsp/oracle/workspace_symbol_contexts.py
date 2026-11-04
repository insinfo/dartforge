"""Compare workspace symbols across nested package catalogs and generated files.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... python3 workspace_symbol_contexts.py
       <output.jsonl> <dartforge-lsp>

Both contexts import the same bridge, which imports different physical versions
of one package. The child also has a private dependency and generated output.
The oracle checks full ordered JSON through two catalog changes, including a
dependency URI that must not replace an explicitly added library's first owner.
Exit status is 1 on any mismatch. This checks correctness, not performance.
"""

import argparse
import copy
import json
import os
from pathlib import Path
import tempfile
import time

from oracle import CAPABILITIES, DART, DART_CACHE, SDK_LIB, Lsp, position


def _write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _package(root, name, source):
    _write(root / "pubspec.yaml", f'name: {name}\nenvironment:\n  sdk: ">=3.6.2 <4.0.0"\n')
    _write(root / "lib/api.dart", source)
    return {"name": name, "rootUri": root.as_uri() + "/",
            "packageUri": "lib/", "languageVersion": "3.6"}


def _run(output, binary, fixture):
    root = fixture / "workspace"
    child = root / "example"
    parent_package = _package(root, "catalog_parent", "class CatalogNeedleParent {}\n")
    child_package = _package(child, "catalog_child", "class CatalogNeedleChild {}\n")
    first = _package(fixture / "version_one", "versioned", "class CatalogNeedleVersionOne {}\n")
    second = _package(fixture / "version_two", "versioned", "class CatalogNeedleVersionTwo {}\n")
    third = _package(fixture / "version_three", "versioned", "class CatalogNeedleVersionThree {}\n")
    shadow = _package(fixture / "shadow_parent", "catalog_parent", "class CatalogNeedleShadowParent {}\n")
    bridge = _package(fixture / "bridge", "bridge", "import 'package:versioned/api.dart';\nclass CatalogNeedleBridge {}\n")
    parent_dependency = _package(fixture / "parent_dependency", "parent_dependency", "class CatalogNeedleParentDependency {}\n")
    child_dependency = _package(fixture / "child_dependency", "child_dependency", "class CatalogNeedleChildDependency {}\n")
    _write(fixture / "version_one/lib/old.dart", "class CatalogNeedleOld {}\n")
    _write(fixture / "version_two/lib/new.dart", "class CatalogNeedleNew {}\n")
    anchor = root / "lib/anchor.dart"
    _write(anchor, "import 'package:bridge/api.dart'; void catalogAnchor() {}\n")
    child_anchor = child / "lib/anchor.dart"
    child_text = "import 'package:bridge/api.dart';\nimport 'package:bridge/adapter.dart';\nimport 'package:catalog_child/generated.dart';\nimport 'package:versioned/api.dart';\nimport 'package:catalog_parent/api.dart';\n"
    _write(child_anchor, child_text)
    _write(child / ".dart_tool/build/generated/catalog_child/lib/generated.dart",
           "class CatalogNeedleGenerated {}\n")
    _write(child / ".dart_tool/build/generated/bridge/lib/adapter.dart",
           "import 'api.dart'; class CatalogNeedleGeneratedBridge {}\n")
    _write(child / "test/runner.dart", "import 'runner.template.dart';\n")
    _write(child / ".dart_tool/build/generated/catalog_child/test/runner.template.dart",
           "class CatalogNeedleGeneratedTest {}\n")
    parent_config = root / ".dart_tool/package_config.json"
    child_config = child / ".dart_tool/package_config.json"
    _write(parent_config, json.dumps({"configVersion": 2, "packages": [
        parent_package, first, bridge, parent_dependency,
    ]}))
    child_packages = [child_package, parent_package, second, bridge, child_dependency]
    _write(child_config, json.dumps({"configVersion": 2, "packages": child_packages}))

    capabilities = copy.deepcopy(CAPABILITIES)
    capabilities["workspace"]["didChangeWatchedFiles"] = {"dynamicRegistration": True}
    env = dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB)
    command = [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]
    if DART_CACHE:
        command.append("--cache=" + DART_CACHE)
    servers = []
    mismatches, total = 0, 0

    def record(phase):
        nonlocal mismatches, total
        results = {}
        for query in ("CatalogNeedle", "CatalogNeedleVersion", "CatalogNeedleBridge",
                      "CatalogNeedleChildDependency", "CatalogNeedleGenerated", "CatalogNeedleOld"):
            row = {"project": "symbol_contexts", "phase": phase,
                   "method": "workspace/symbol", "pos": query}
            for server in servers:
                row[server.name], _ = server.request("workspace/symbol", {"query": query})
                if not isinstance(row[server.name], list):
                    raise RuntimeError(f"{phase} {query} {server.name}: {row[server.name]}")
            same = row["dart"] == row["dartforge"]
            total += 1
            mismatches += not same
            results[query] = row["dart"]
            output.write(json.dumps(row, ensure_ascii=False) + "\n")
            output.flush()
            print(f"{phase}: {query}: {'equal' if same else 'DIFFERENT'} "
                  f"({len(row['dart'])}/{len(row['dartforge'])})", flush=True)
        # These assertions guard the reference fixture, not DartForge's result.
        assert results["CatalogNeedleChildDependency"], "the child catalog was not analyzed"
        assert results["CatalogNeedleGenerated"], "the generated child library was not found"
        versions = results["CatalogNeedleVersion"]
        assert len(versions) == 1, versions
        assert versions[0]["name"] == ("CatalogNeedleVersionThree" if phase in ("catalog_changed", "added_owner")
                                       else "CatalogNeedleVersionTwo"), versions
        assert "CatalogNeedleShadowParent" not in [s["name"] for s in results["CatalogNeedle"]], \
            "a known dependency replaced the explicitly added parent library"

    try:
        servers = [Lsp("dart", command), Lsp("dartforge", [binary, "--stdio"], env=env)]
        for server in servers:
            response, _ = server.request("initialize", {
                "processId": os.getpid(), "rootUri": root.as_uri(),
                "workspaceFolders": [{"uri": root.as_uri(), "name": "catalog_workspace"}],
                "capabilities": capabilities,
                "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True},
            })
            if not isinstance(response, dict) or "capabilities" not in response:
                raise RuntimeError(f"{server.name}: initialize failed: {response}")
            server.notify("initialized", {})
        previous = servers[0].analysis_completions
        for server in servers:
            server.notify("textDocument/didOpen", {"textDocument": {
                "uri": anchor.as_uri(), "languageId": "dart", "version": 1,
                "text": anchor.read_text(encoding="utf-8"),
            }})
        if not servers[0].wait_analysis(previous, limit=60, quiet=1.5):
            raise RuntimeError("initial analysis did not finish")
        record("initial")
        record("repeat")
        for phase in ("catalog_changed", "added_owner"):
            child_packages[2] = third
            if phase == "added_owner":
                child_packages[1] = shadow
            _write(child_config, json.dumps({"configVersion": 2, "packages": child_packages}))
            previous = servers[0].analysis_completions
            for server in servers:
                server.notify("workspace/didChangeWatchedFiles", {"changes": [{
                    "uri": child_config.as_uri(), "type": 2,
                }]})
            if not servers[0].wait_analysis(previous, limit=60, quiet=1.5):
                raise RuntimeError("changed catalog analysis did not finish")
            # Status from resolution of a preceding query can arrive after the
            # write. Probe an import's definition in the changed catalog too.
            # This readiness request is part of the scenario's LSP sequence.
            deadline = time.monotonic() + 30
            while True:
                target = third if phase == "catalog_changed" else shadow
                offset = child_text.index(f"package:{target['name']}/api.dart") + len("package:")
                params = {
                    "textDocument": {"uri": child_anchor.as_uri()}, "position": position(child_text, offset),
                }
                # Keep the client request sequence identical in both servers,
                # even though the readiness assertion concerns the reference.
                probes = [server.request("textDocument/definition", params)[0] for server in servers]
                definitions = probes[0]
                locations = definitions if isinstance(definitions, list) else [definitions]
                expected = ((fixture / "version_three") if phase == "catalog_changed" else (fixture / "shadow_parent")) / "lib/api.dart"
                if any(isinstance(d, dict) and (d.get("uri") or d.get("targetUri")) == expected.as_uri() for d in locations):
                    break
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"changed catalog was not applied: {definitions}")
                time.sleep(0.1)
            record(phase)
    finally:
        for server in servers:
            server.close()
    print(f"complete ordered responses: {total - mismatches}/{total}", flush=True)
    return bool(mismatches)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output")
    parser.add_argument("binary")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="dartforge-symbol-contexts-") as temporary:
        with open(args.output, "w", encoding="utf-8") as output:
            return _run(output, os.path.abspath(args.binary), Path(temporary).resolve())


if __name__ == "__main__":
    raise SystemExit(main())
