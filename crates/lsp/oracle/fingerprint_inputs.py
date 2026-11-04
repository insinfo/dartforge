"""Fingerprint a project's Dart/configuration inputs and resolved package roots.

Usage: python3 fingerprint_inputs.py <project> <sdk> <output.json>
The manifest includes content hashes and modification times. Missing roots are
recorded explicitly and cause a failing exit status. Unreadable files or files
changing during hashing also fail.
Generated Dart files under .dart_tool are included. Git and node_modules trees
are excluded unless a package root points into them directly.
"""

import hashlib
import json
import os
from pathlib import Path
import sys
from urllib.parse import urljoin, urlsplit
from urllib.request import url2pathname


def _walk(root):
    def fail(error):
        raise error

    for directory, children, files in os.walk(root, onerror=fail):
        children[:] = [c for c in children if c not in (".git", "node_modules")]
        for name in files:
            yield Path(directory) / name


def _file_uri(value):
    parsed = urlsplit(value)
    if parsed.scheme != "file":
        raise ValueError(f"package root is not a file URI: {value}")
    path = url2pathname(parsed.path)
    if parsed.netloc and parsed.netloc != "localhost":
        path = f"//{parsed.netloc}{path}"
    return Path(os.path.normpath(path))


def main():
    if len(sys.argv) != 4:
        raise SystemExit("Usage: fingerprint_inputs.py <project> <sdk> <output.json>")
    project, sdk, output = (Path(p).absolute() for p in sys.argv[1:])
    roots = {project, sdk / "lib"}
    for config in _walk(project):
        if config.name != "package_config.json" or config.parent.name != ".dart_tool":
            continue
        data = json.loads(config.read_text(encoding="utf-8"))
        for package in data["packages"]:
            roots.add(_file_uri(urljoin(config.as_uri(), package["rootUri"])))
    paths = set()
    missing = []
    names = {"package_config.json", "libraries.json", ".packages", "pubspec.lock"}
    for root in sorted(roots):
        if not root.is_dir():
            missing.append(str(root))
            continue
        for path in _walk(root):
            if path.suffix in (".dart", ".yaml", ".yml") or path.name in names:
                paths.add(path)
    paths.update((sdk / "version", sdk / "bin" / ("dart.exe" if os.name == "nt" else "dart"),
                  sdk / "bin/snapshots/analysis_server.dart.snapshot"))
    files = []
    for path in sorted(paths):
        before = path.stat()
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        after = path.stat()
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise RuntimeError(f"input changed while fingerprinting: {path}")
        files.append({"path": str(path), "size": after.st_size,
                      "mtime_ns": after.st_mtime_ns, "sha256": digest})
    manifest = {"roots": [str(p) for p in sorted(roots)], "missingRoots": missing, "files": files}
    encoded = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    manifest["digest"] = hashlib.sha256(encoded).hexdigest()
    output.write_text(json.dumps(manifest, ensure_ascii=False), encoding="utf-8")
    print(f"{manifest['digest']} files={len(files)} missingRoots={len(missing)}", flush=True)
    if missing:
        raise SystemExit(f"error: {len(missing)} package roots are unavailable; see {output}")


if __name__ == "__main__":
    main()
