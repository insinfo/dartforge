# LSP parity oracle: opens the same Dart files in `dart language-server`
# (SDK 3.6.2) and in `dartforge-lsp`, requests the same features at the same
# positions and writes the responses side by side (JSONL).
#
# Usage: python oracle.py <output.jsonl> <dartforge-binary> [project ...]
# (environment variables: ORACLE_DART, ORACLE_SDK_LIB, ORACLE_PER_FILE,
# ORACLE_SNIPPETS, ORACLE_METHODS)
# The summary (parity table) comes out of `summary.py`.
import json, os, re, subprocess, sys, threading, time, queue, urllib.parse, pathlib

# Paths of the 3.6.2 SDK: `ORACLE_DART` (the `dart` executable) and
# `ORACLE_SDK_LIB` (the `lib` folder of the same SDK, which becomes the
# `DARTFORGE_SDK_LIB` of the DartForge server). Without them, the Windows
# defaults.
DART = os.environ.get("ORACLE_DART", r"C:\tools\dartsdk-3.6.2\bin\dart.exe")
SDK_LIB = os.environ.get("ORACLE_SDK_LIB", r"C:\tools\dartsdk-3.6.2\lib")
PER_FILE = int(os.environ.get("ORACLE_PER_FILE", "12"))
# Only these methods (comma separated, without `textDocument/`); empty: all.
METHODS = {m.strip() for m in os.environ.get("ORACLE_METHODS", "").split(",") if m.strip()}


def requested(method):
    return not METHODS or method.split("/", 1)[-1] in METHODS


class Lsp:
    def __init__(self, name, cmd, env=None):
        self.name = name
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, env=env)
        self.next_id = 0
        self.responses = {}
        self.cv = threading.Condition()
        self.diagnostics = {}
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        f = self.p.stdout
        while True:
            headers = {}
            while True:
                line = f.readline()
                if not line:
                    return
                line = line.decode().strip()
                if not line:
                    break
                k, v = line.split(":", 1)
                headers[k.lower()] = v.strip()
            body = f.read(int(headers["content-length"]))
            m = json.loads(body)
            if "method" in m and "id" in m:
                # Request from the server to the client.
                r = None
                if m["method"] == "workspace/configuration":
                    r = [{} for _ in m["params"]["items"]]
                self._send({"jsonrpc": "2.0", "id": m["id"], "result": r})
            elif "method" in m:
                if m["method"] == "textDocument/publishDiagnostics":
                    with self.cv:
                        self.diagnostics[norm_uri(m["params"]["uri"])] = m["params"]["diagnostics"]
                        self.cv.notify_all()
            else:
                with self.cv:
                    self.responses[m["id"]] = m
                    self.cv.notify_all()

    def _send(self, m):
        b = json.dumps(m).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
        self.p.stdin.flush()

    def request(self, method, params, limit=120):
        self.next_id += 1
        i = self.next_id
        t0 = time.perf_counter()
        self._send({"jsonrpc": "2.0", "id": i, "method": method, "params": params})
        with self.cv:
            while i not in self.responses:
                if not self.cv.wait(timeout=limit):
                    return {"error": "timed out"}, limit * 1000
            r = self.responses.pop(i)
        ms = (time.perf_counter() - t0) * 1000
        if "error" in r:
            return {"error": r["error"]}, ms
        return r.get("result"), ms

    def notify(self, method, params):
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def close(self):
        try:
            self.request("shutdown", None, limit=10)
            self.notify("exit", None)
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()


def norm_uri(u):
    if not isinstance(u, str):
        return u
    u = urllib.parse.unquote(u)
    return u.lower().replace("file:///", "file:///")


CAPABILITIES = {
    "textDocument": {
        "synchronization": {"didSave": True},
        "completion": {"completionItem": {"snippetSupport": True, "documentationFormat": ["markdown", "plaintext"],
                                          "labelDetailsSupport": True,
                                          "resolveSupport": {"properties": ["documentation"]}},
                       "contextSupport": True},
        "hover": {"contentFormat": ["markdown", "plaintext"]},
        "signatureHelp": {"signatureInformation": {"documentationFormat": ["markdown", "plaintext"],
                                                   "parameterInformation": {"labelOffsetSupport": True},
                                                   "activeParameterSupport": True},
                          "contextSupport": True},
        "definition": {}, "typeDefinition": {}, "implementation": {}, "references": {},
        "documentHighlight": {}, "documentSymbol": {"hierarchicalDocumentSymbolSupport": True},
        "codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": [
            "", "quickfix", "refactor", "refactor.extract", "refactor.inline", "refactor.rewrite", "source",
            "source.organizeImports", "source.fixAll"]}}, "isPreferredSupport": True},
        "formatting": {}, "rangeFormatting": {}, "onTypeFormatting": {},
        "rename": {"prepareSupport": True},
        "foldingRange": {"lineFoldingOnly": True},
        "selectionRange": {},
        "semanticTokens": {"requests": {"full": True, "range": True},
                           "tokenTypes": ["namespace", "type", "class", "enum", "interface", "struct", "typeParameter",
                                          "parameter", "variable", "property", "enumMember", "event", "function",
                                          "method", "macro", "keyword", "modifier", "comment", "string", "number",
                                          "regexp", "operator", "decorator"],
                           "tokenModifiers": ["declaration", "definition", "readonly", "static", "deprecated",
                                              "abstract", "async", "modification", "documentation", "defaultLibrary"],
                           "formats": ["relative"]},
        "inlayHint": {}, "callHierarchy": {}, "typeHierarchy": {}, "documentLink": {}, "colorProvider": {},
        "publishDiagnostics": {"relatedInformation": True, "versionSupport": True},
    },
    "workspace": {"workspaceEdit": {"documentChanges": True, "resourceOperations": ["create", "rename", "delete"]},
                  "applyEdit": True, "configuration": True, "workspaceFolders": True, "symbol": {},
                  "executeCommand": {}},
    "window": {"workDoneProgress": False},
}
# The code actions' `SnippetTextEdit` (what Dart-Code announces): on by
# default; `ORACLE_SNIPPETS=0` measures the client without the capability.
if os.environ.get("ORACLE_SNIPPETS", "1") != "0":
    CAPABILITIES["experimental"] = {"snippetTextEdit": True}

ID = re.compile(r"[A-Za-z_$][A-Za-z0-9_$]*")
KEYWORDS = set("""abstract as assert async await base break case catch class const continue covariant default deferred do
dynamic else enum export extends extension external factory false final finally for Function get hide if implements import
in interface is late library mixin new null of on operator part required rethrow return sealed set show static super switch
sync this throw true try type typedef var void when while with yield""".split())


def code_tokens(t):
    """Identifiers outside comments and strings: (start, end, text)."""
    output, i, n = [], 0, len(t)
    while i < n:
        c = t[i]
        if t.startswith("//", i):
            j = t.find("\n", i)
            i = n if j < 0 else j
        elif t.startswith("/*", i):
            j = t.find("*/", i + 2)
            i = n if j < 0 else j + 2
        elif c in "'\"":
            tri = t[i:i + 3]
            if tri in ("'''", '"""'):
                j = t.find(tri, i + 3)
                i = n if j < 0 else j + 3
            else:
                j = i + 1
                while j < n and t[j] != c and t[j] != "\n":
                    j += 2 if t[j] == "\\" else 1
                i = j + 1
        elif c.isalpha() or c in "_$":
            m = ID.match(t, i)
            # raw `r'...'`
            if m.group() == "r" and m.end() < n and t[m.end()] in "'\"":
                i = m.end()
                continue
            output.append((i, m.end(), m.group()))
            i = m.end()
        elif c.isdigit():
            while i < n and (t[i].isalnum() or t[i] == "."):
                i += 1
        else:
            i += 1
    return output


def position(t, off):
    line = t.count("\n", 0, off)
    start = t.rfind("\n", 0, off) + 1
    col = len(t[start:off].encode("utf-16-le")) // 2
    return {"line": line, "character": col}


def sample(items, k):
    if len(items) <= k:
        return items
    step = len(items) / k
    return [items[int(i * step)] for i in range(k)]


def main():
    output_path = sys.argv[1]
    binary = sys.argv[2]
    projects = sys.argv[3:]
    env = dict(os.environ)
    env["DARTFORGE_SDK_LIB"] = SDK_LIB
    with open(output_path, "w", encoding="utf-8") as out:
        for proj in projects:
            root = pathlib.Path(proj).resolve()
            root_uri = root.as_uri()
            files = sorted(p for d in ("lib", "bin") for p in (root / d).rglob("*.dart"))
            servers = [
                Lsp("dart", [DART, "language-server", "--protocol=lsp", "--client-id=oracle"]),
                Lsp("dartforge", [binary, "--stdio"], env=env),
            ]
            for s in servers:
                r, ms = s.request("initialize", {"processId": os.getpid(), "rootUri": root_uri,
                                                 "workspaceFolders": [{"uri": root_uri, "name": root.name}],
                                                 "capabilities": CAPABILITIES,
                                                 "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True}})
                out.write(json.dumps({"project": root.name, "server": s.name, "method": "initialize",
                                      "result": r}) + "\n")
                s.notify("initialized", {})
            texts = {}
            for a in files:
                t = a.read_text(encoding="utf-8")
                texts[a] = t
                for s in servers:
                    s.notify("textDocument/didOpen", {"textDocument": {"uri": a.as_uri(), "languageId": "dart",
                                                                       "version": 1, "text": t}})
            # Waits for Dart's diagnostics (initial analysis).
            deadline = time.time() + 90
            dart = servers[0]
            with dart.cv:
                while len(dart.diagnostics) < len(files) and time.time() < deadline:
                    dart.cv.wait(timeout=1)
            time.sleep(1.0)
            for a in files:
                t = texts[a]
                uri = a.as_uri()
                doc = {"uri": uri}
                rel = str(a.relative_to(root)).replace("\\", "/")

                def record(method, pos, params, extra=None):
                    if not requested(method):
                        return
                    line = {"project": root.name, "file": rel, "method": method, "pos": pos}
                    if extra:
                        line.update(extra)
                    for s in servers:
                        r, ms = s.request(method, params)
                        line[s.name] = r
                        line["ms_" + s.name] = ms
                    out.write(json.dumps(line, ensure_ascii=False) + "\n")
                    out.flush()

                # Per file.
                for method, params in [
                    ("textDocument/documentSymbol", {"textDocument": doc}),
                    ("textDocument/foldingRange", {"textDocument": doc}),
                    ("textDocument/semanticTokens/full", {"textDocument": doc}),
                    ("textDocument/inlayHint", {"textDocument": doc, "range": {"start": {"line": 0, "character": 0},
                                                                               "end": position(t, len(t))}}),
                    ("textDocument/documentLink", {"textDocument": doc}),
                    ("textDocument/formatting", {"textDocument": doc, "options": {"tabSize": 2, "insertSpaces": True}}),
                ]:
                    record(method, None, params)
                # Dart's diagnostics: actions in the range of each one.
                for d in dart.diagnostics.get(norm_uri(uri), [])[:15]:
                    record("textDocument/codeAction", d["range"]["start"],
                           {"textDocument": doc, "range": d["range"], "context": {"diagnostics": [d]}},
                           {"diagnostic": d.get("code")})
                ids = [x for x in code_tokens(t) if x[2] not in KEYWORDS]
                for (i0, i1, name) in sample(ids, PER_FILE):
                    pos = position(t, i0 + min(1, i1 - i0 - 1))
                    pp = {"textDocument": doc, "position": pos}
                    ex = {"name": name}
                    record("textDocument/hover", pos, pp, ex)
                    record("textDocument/definition", pos, pp, ex)
                    record("textDocument/typeDefinition", pos, pp, ex)
                    record("textDocument/implementation", pos, pp, ex)
                    record("textDocument/references", pos, dict(pp, context={"includeDeclaration": True}), ex)
                    record("textDocument/documentHighlight", pos, pp, ex)
                    record("textDocument/prepareRename", pos, pp, ex)
                    record("textDocument/rename", pos, dict(pp, newName=name + "New"), ex)
                    record("textDocument/codeAction", pos,
                           {"textDocument": doc, "range": {"start": position(t, i0), "end": position(t, i0)},
                            "context": {"diagnostics": []}}, ex)
                    record("textDocument/selectionRange", pos, {"textDocument": doc, "positions": [pos]}, ex)
                    record("textDocument/prepareCallHierarchy", pos, pp, ex)
                    record("textDocument/prepareTypeHierarchy", pos, pp, ex)
                    # Completion: in the middle of the name (a prefix of up to 2)
                    # or right after the dot of a member access.
                    if i0 > 0 and t[i0 - 1] == ".":
                        cpos = position(t, i0)
                        ctx = {"triggerKind": 2, "triggerCharacter": "."}
                        prefix = ""
                    else:
                        cut = i0 + min(2, i1 - i0)
                        cpos = position(t, cut)
                        ctx = {"triggerKind": 1}
                        prefix = t[i0:cut]
                    record("textDocument/completion", cpos,
                           {"textDocument": doc, "position": cpos, "context": ctx},
                           {"name": name, "prefix": prefix})
                # Signature help: right after the `(` of a call and after its
                # first comma.
                calls = [m.end() for m in re.finditer(r"[A-Za-z0-9_$>]\(", t)]
                codes = set()
                for (a0, a1, _) in ids:
                    codes.add(a0)
                for off in sample(calls, max(3, PER_FILE // 3)):
                    pos = position(t, off)
                    record("textDocument/signatureHelp", pos,
                           {"textDocument": doc, "position": pos,
                            "context": {"triggerKind": 2, "triggerCharacter": "(", "isRetrigger": False}})
                    comma = t.find(",", off, off + 80)
                    close = t.find(")", off)
                    if 0 <= comma < (close if close >= 0 else len(t)):
                        pos = position(t, comma + 1)
                        record("textDocument/signatureHelp", pos,
                               {"textDocument": doc, "position": pos,
                                "context": {"triggerKind": 1, "isRetrigger": False}})
            # workspace/symbol
            for q in ["Arg", "parse", "Scanner", "ctx", "Ponto", "join"] if requested("workspace/symbol") else []:
                line = {"project": root.name, "method": "workspace/symbol", "pos": q}
                for s in servers:
                    r, ms = s.request("workspace/symbol", {"query": q})
                    line[s.name] = r
                    line["ms_" + s.name] = ms
                out.write(json.dumps(line, ensure_ascii=False) + "\n")
            for s in servers:
                s.close()
            print("project", root.name, "done", flush=True)


if __name__ == "__main__":
    main()
