# Oráculo de paridade do LSP: abre os mesmos arquivos Dart no
# `dart language-server` (SDK 3.6.2) e no `dartforge-lsp`, pede os mesmos
# recursos nas mesmas posições e grava as respostas lado a lado (JSONL).
#
# Uso: python oraculo.py <saida.jsonl> <binario-dartforge> [projeto ...]
# O resumo (tabela de paridade) sai de `resumo.py`.
import json, os, re, subprocess, sys, threading, time, queue, urllib.parse, pathlib

DART = r"C:\tools\dartsdk-3.6.2\bin\dart.exe"
SDK_LIB = r"C:\tools\dartsdk-3.6.2\lib"
POR_ARQUIVO = int(os.environ.get("ORACULO_POR_ARQUIVO", "12"))


class Lsp:
    def __init__(self, nome, cmd, env=None):
        self.nome = nome
        self.p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, env=env)
        self.prox = 0
        self.respostas = {}
        self.cv = threading.Condition()
        self.diagnosticos = {}
        threading.Thread(target=self._ler, daemon=True).start()

    def _ler(self):
        f = self.p.stdout
        while True:
            cab = {}
            while True:
                linha = f.readline()
                if not linha:
                    return
                linha = linha.decode().strip()
                if not linha:
                    break
                k, v = linha.split(":", 1)
                cab[k.lower()] = v.strip()
            corpo = f.read(int(cab["content-length"]))
            m = json.loads(corpo)
            if "method" in m and "id" in m:
                # Pedido do servidor ao cliente.
                r = None
                if m["method"] == "workspace/configuration":
                    r = [{} for _ in m["params"]["items"]]
                self._enviar({"jsonrpc": "2.0", "id": m["id"], "result": r})
            elif "method" in m:
                if m["method"] == "textDocument/publishDiagnostics":
                    with self.cv:
                        self.diagnosticos[norm_uri(m["params"]["uri"])] = m["params"]["diagnostics"]
                        self.cv.notify_all()
            else:
                with self.cv:
                    self.respostas[m["id"]] = m
                    self.cv.notify_all()

    def _enviar(self, m):
        b = json.dumps(m).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
        self.p.stdin.flush()

    def pedir(self, metodo, params, limite=120):
        self.prox += 1
        i = self.prox
        t0 = time.perf_counter()
        self._enviar({"jsonrpc": "2.0", "id": i, "method": metodo, "params": params})
        with self.cv:
            while i not in self.respostas:
                if not self.cv.wait(timeout=limite):
                    return {"erro": "tempo esgotado"}, limite * 1000
            r = self.respostas.pop(i)
        ms = (time.perf_counter() - t0) * 1000
        if "error" in r:
            return {"erro": r["error"]}, ms
        return r.get("result"), ms

    def notificar(self, metodo, params):
        self._enviar({"jsonrpc": "2.0", "method": metodo, "params": params})

    def fechar(self):
        try:
            self.pedir("shutdown", None, limite=10)
            self.notificar("exit", None)
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()


def norm_uri(u):
    if not isinstance(u, str):
        return u
    u = urllib.parse.unquote(u)
    return u.lower().replace("file:///", "file:///")


CAPACIDADES = {
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

ID = re.compile(r"[A-Za-z_$][A-Za-z0-9_$]*")
PALAVRAS = set("""abstract as assert async await base break case catch class const continue covariant default deferred do
dynamic else enum export extends extension external factory false final finally for Function get hide if implements import
in interface is late library mixin new null of on operator part required rethrow return sealed set show static super switch
sync this throw true try type typedef var void when while with yield""".split())


def tokens_de_codigo(t):
    """Identificadores fora de comentários e strings: (início, fim, texto)."""
    saida, i, n = [], 0, len(t)
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
            # `r'...'` cru
            if m.group() == "r" and m.end() < n and t[m.end()] in "'\"":
                i = m.end()
                continue
            saida.append((i, m.end(), m.group()))
            i = m.end()
        elif c.isdigit():
            while i < n and (t[i].isalnum() or t[i] == "."):
                i += 1
        else:
            i += 1
    return saida


def posicao(t, off):
    linha = t.count("\n", 0, off)
    ini = t.rfind("\n", 0, off) + 1
    col = len(t[ini:off].encode("utf-16-le")) // 2
    return {"line": linha, "character": col}


def amostrar(lista, k):
    if len(lista) <= k:
        return lista
    passo = len(lista) / k
    return [lista[int(i * passo)] for i in range(k)]


def main():
    saida = sys.argv[1]
    binario = sys.argv[2]
    projetos = sys.argv[3:]
    env = dict(os.environ)
    env["DARTFORGE_SDK_LIB"] = SDK_LIB
    with open(saida, "w", encoding="utf-8") as out:
        for proj in projetos:
            raiz = pathlib.Path(proj).resolve()
            raiz_uri = raiz.as_uri()
            arquivos = sorted(p for d in ("lib", "bin") for p in (raiz / d).rglob("*.dart"))
            servidores = [
                Lsp("dart", [DART, "language-server", "--protocol=lsp", "--client-id=oraculo"]),
                Lsp("dartforge", [binario, "--stdio"], env=env),
            ]
            for s in servidores:
                r, ms = s.pedir("initialize", {"processId": os.getpid(), "rootUri": raiz_uri,
                                               "workspaceFolders": [{"uri": raiz_uri, "name": raiz.name}],
                                               "capabilities": CAPACIDADES,
                                               "initializationOptions": {"onlyAnalyzeProjectsWithOpenFiles": True}})
                out.write(json.dumps({"projeto": raiz.name, "servidor": s.nome, "metodo": "initialize",
                                      "resultado": r}) + "\n")
                s.notificar("initialized", {})
            textos = {}
            for a in arquivos:
                t = a.read_text(encoding="utf-8")
                textos[a] = t
                for s in servidores:
                    s.notificar("textDocument/didOpen", {"textDocument": {"uri": a.as_uri(), "languageId": "dart",
                                                                          "version": 1, "text": t}})
            # Espera os diagnósticos do Dart (análise inicial).
            fim = time.time() + 90
            dart = servidores[0]
            with dart.cv:
                while len(dart.diagnosticos) < len(arquivos) and time.time() < fim:
                    dart.cv.wait(timeout=1)
            time.sleep(1.0)
            for a in arquivos:
                t = textos[a]
                uri = a.as_uri()
                doc = {"uri": uri}
                rel = str(a.relative_to(raiz)).replace("\\", "/")

                def registrar(metodo, pos, params, extra=None):
                    linha = {"projeto": raiz.name, "arquivo": rel, "metodo": metodo, "pos": pos}
                    if extra:
                        linha.update(extra)
                    for s in servidores:
                        r, ms = s.pedir(metodo, params)
                        linha[s.nome] = r
                        linha["ms_" + s.nome] = ms
                    out.write(json.dumps(linha, ensure_ascii=False) + "\n")
                    out.flush()

                # Por arquivo.
                for metodo, params in [
                    ("textDocument/documentSymbol", {"textDocument": doc}),
                    ("textDocument/foldingRange", {"textDocument": doc}),
                    ("textDocument/semanticTokens/full", {"textDocument": doc}),
                    ("textDocument/inlayHint", {"textDocument": doc, "range": {"start": {"line": 0, "character": 0},
                                                                               "end": posicao(t, len(t))}}),
                    ("textDocument/documentLink", {"textDocument": doc}),
                    ("textDocument/formatting", {"textDocument": doc, "options": {"tabSize": 2, "insertSpaces": True}}),
                ]:
                    registrar(metodo, None, params)
                # Diagnósticos do Dart: ações no intervalo de cada um.
                for d in dart.diagnosticos.get(norm_uri(uri), [])[:15]:
                    registrar("textDocument/codeAction", d["range"]["start"],
                              {"textDocument": doc, "range": d["range"], "context": {"diagnostics": [d]}},
                              {"diagnostico": d.get("code")})
                ids = [x for x in tokens_de_codigo(t) if x[2] not in PALAVRAS]
                for (i0, i1, nome) in amostrar(ids, POR_ARQUIVO):
                    pos = posicao(t, i0 + min(1, i1 - i0 - 1))
                    pp = {"textDocument": doc, "position": pos}
                    ex = {"nome": nome}
                    registrar("textDocument/hover", pos, pp, ex)
                    registrar("textDocument/definition", pos, pp, ex)
                    registrar("textDocument/typeDefinition", pos, pp, ex)
                    registrar("textDocument/implementation", pos, pp, ex)
                    registrar("textDocument/references", pos, dict(pp, context={"includeDeclaration": True}), ex)
                    registrar("textDocument/documentHighlight", pos, pp, ex)
                    registrar("textDocument/prepareRename", pos, pp, ex)
                    registrar("textDocument/rename", pos, dict(pp, newName=nome + "Novo"), ex)
                    registrar("textDocument/codeAction", pos,
                              {"textDocument": doc, "range": {"start": posicao(t, i0), "end": posicao(t, i0)},
                               "context": {"diagnostics": []}}, ex)
                    registrar("textDocument/selectionRange", pos, {"textDocument": doc, "positions": [pos]}, ex)
                    registrar("textDocument/prepareCallHierarchy", pos, pp, ex)
                    registrar("textDocument/prepareTypeHierarchy", pos, pp, ex)
                    # Completar: no meio do nome (prefixo de até 2) ou logo
                    # depois do ponto de um acesso a membro.
                    if i0 > 0 and t[i0 - 1] == ".":
                        cpos = posicao(t, i0)
                        ctx = {"triggerKind": 2, "triggerCharacter": "."}
                        prefixo = ""
                    else:
                        corte = i0 + min(2, i1 - i0)
                        cpos = posicao(t, corte)
                        ctx = {"triggerKind": 1}
                        prefixo = t[i0:corte]
                    registrar("textDocument/completion", cpos,
                              {"textDocument": doc, "position": cpos, "context": ctx},
                              {"nome": nome, "prefixo": prefixo})
                # Ajuda de assinatura: logo depois de `(` de uma chamada e
                # depois da primeira vírgula dela.
                chamadas = [m.end() for m in re.finditer(r"[A-Za-z0-9_$>]\(", t)]
                cods = set()
                for (a0, a1, _) in ids:
                    cods.add(a0)
                for off in amostrar(chamadas, max(3, POR_ARQUIVO // 3)):
                    pos = posicao(t, off)
                    registrar("textDocument/signatureHelp", pos,
                              {"textDocument": doc, "position": pos,
                               "context": {"triggerKind": 2, "triggerCharacter": "(", "isRetrigger": False}})
                    virg = t.find(",", off, off + 80)
                    fecha = t.find(")", off)
                    if 0 <= virg < (fecha if fecha >= 0 else len(t)):
                        pos = posicao(t, virg + 1)
                        registrar("textDocument/signatureHelp", pos,
                                  {"textDocument": doc, "position": pos,
                                   "context": {"triggerKind": 1, "isRetrigger": False}})
            # workspace/symbol
            for q in ["Arg", "parse", "Scanner", "ctx", "Ponto", "join"]:
                linha = {"projeto": raiz.name, "metodo": "workspace/symbol", "pos": q}
                for s in servidores:
                    r, ms = s.pedir("workspace/symbol", {"query": q})
                    linha[s.nome] = r
                    linha["ms_" + s.nome] = ms
                out.write(json.dumps(linha, ensure_ascii=False) + "\n")
            for s in servidores:
                s.fechar()
            print("projeto", raiz.name, "feito", flush=True)


if __name__ == "__main__":
    main()
