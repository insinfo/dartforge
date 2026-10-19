"""Capture Dart LSP suggestions: python3 check-dart-oracle.py SDK OUTPUT_JSON."""

import json
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import time


def capture(sdk, output):
    with tempfile.TemporaryDirectory(prefix="uri oracle ") as temporary:
        root = Path(temporary)
        lib = root / "lib"
        lib.mkdir()
        (root / ".dart_tool").mkdir()
        (root / "pubspec.yaml").write_text(
            'name: uri_fixture\nenvironment:\n  sdk: ">=3.6.0 <4.0.0"\n'
        )
        package = {
            "name": "uri_fixture", "rootUri": "../", "packageUri": "lib/",
            "languageVersion": "3.6",
        }
        (root / ".dart_tool/package_config.json").write_text(
            json.dumps({"configVersion": 2, "packages": [package]})
        )
        for folder, filename in [
            ("folder space", "hello.dart"),
            ("folder space", "fancy é name.dart"),
            ("percent%literal", "p.dart"),
            ("hash#tag", "h.dart"),
        ]:
            directory = lib / folder
            directory.mkdir(exist_ok=True)
            (directory / filename).write_text("class Example {}\n")
        source = lib / "main.dart"
        source.write_text("void main() {}\n")
        uri = source.as_uri()
        server = subprocess.Popen(
            [str(sdk / "bin/dart"), "language-server", "--protocol=lsp"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        messages = queue.Queue()

        def read_messages():
            while True:
                headers = {}
                while True:
                    line = server.stdout.readline()
                    if not line:
                        return
                    if line in (b"\r\n", b"\n"):
                        break
                    key, value = line.decode().split(":", 1)
                    headers[key.lower()] = value.strip()
                data = server.stdout.read(int(headers["content-length"]))
                messages.put(json.loads(data))

        threading.Thread(target=read_messages, daemon=True).start()

        def send(method, params=None, request_id=None):
            request = {"jsonrpc": "2.0", "method": method}
            if params is not None:
                request["params"] = params
            if request_id is not None:
                request["id"] = request_id
            data = json.dumps(request).encode()
            server.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
            server.stdin.flush()

        def response(request_id):
            deadline = time.monotonic() + 35
            while True:
                item = messages.get(timeout=max(.01, deadline - time.monotonic()))
                if item.get("id") == request_id:
                    if "error" in item:
                        raise RuntimeError(item["error"])
                    return item

        def normalize(value):
            return (value or "").replace(lib.as_uri(), "file:<fixture>")

        try:
            send("initialize", {
                "processId": None, "rootUri": root.as_uri(), "capabilities": {},
                "workspaceFolders": [{"uri": root.as_uri(), "name": "uri_fixture"}],
            }, 1)
            response(1)
            send("initialized", {})
            send("textDocument/didOpen", {"textDocument": {
                "uri": uri, "languageId": "dart", "version": 1,
                "text": source.read_text(),
            }})
            rows = []
            partials = [
                "folder%20space/", "folder space/", "percent%25literal/", "hash%23tag/",
                "package:uri_fixture/folder%20space/", (lib / "folder space").as_uri() + "/",
            ]
            for request_id, partial in enumerate(partials, 2):
                text = f"import '{partial}';\nvoid main() {{}}\n"
                source.write_text(text)
                send("textDocument/didChange", {
                    "textDocument": {"uri": uri, "version": request_id},
                    "contentChanges": [{"text": text}],
                })
                send("textDocument/completion", {
                    "textDocument": {"uri": uri},
                    "position": {"line": 0, "character": len("import '") + len(partial)},
                }, request_id)
                result = response(request_id)
                items = result.get("result") or []
                if isinstance(items, dict):
                    items = items.get("items", [])
                rows.append({
                    "partial": normalize(partial),
                    "items": [{
                        "label": normalize(item.get("label")),
                        "newText": normalize(item.get("textEdit", {}).get(
                            "newText", item.get("insertText")
                        )),
                    } for item in items],
                    "error": result.get("error"),
                })
            output.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + "\n")
            send("shutdown", request_id=100)
            response(100)
            send("exit")
        finally:
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()


if __name__ == "__main__":
    capture(Path(sys.argv[1]).resolve(), Path(sys.argv[2]))
