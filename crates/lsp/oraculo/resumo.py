# Tabela de paridade a partir do JSONL do oráculo.
# Uso: python resumo.py <resultados.jsonl> [--exemplos N] [--metodo M]
import json, sys, re, collections, urllib.parse

args = sys.argv[1:]
arq = args[0]
N_EX = int(args[args.index("--exemplos") + 1]) if "--exemplos" in args else 2
SO = args[args.index("--metodo") + 1] if "--metodo" in args else None


def nu(u):
    return urllib.parse.unquote(u or "").lower()


def rg(r):
    if not r:
        return None
    return (r["start"]["line"], r["start"]["character"], r["end"]["line"], r["end"]["character"])


def erro(x):
    return isinstance(x, dict) and "erro" in x


def locais(x):
    if x is None or erro(x):
        return frozenset()
    if isinstance(x, dict):
        x = [x]
    s = set()
    for l in x:
        if "targetUri" in l:
            s.add((nu(l["targetUri"]), rg(l["targetSelectionRange"])))
        else:
            s.add((nu(l["uri"]), rg(l["range"])))
    return frozenset(s)


def texto_hover(x):
    if not x or erro(x):
        return None
    c = x.get("contents")
    if isinstance(c, dict):
        c = c.get("value")
    elif isinstance(c, list):
        c = "\n".join(i if isinstance(i, str) else i.get("value", "") for i in c)
    return re.sub(r"\s+", " ", c or "").strip() or None


def assinatura_hover(x):
    t = texto_hover(x)
    if t is None:
        return None
    m = re.search(r"```dart (.*?) ```", t)
    return m.group(1) if m else t


def edicoes(x):
    if not x or erro(x):
        return None if not erro(x) else "ERRO"
    s = set()
    for dc in x.get("documentChanges") or []:
        if "textDocument" in dc:
            for e in dc["edits"]:
                s.add((nu(dc["textDocument"]["uri"]), rg(e["range"]), e["newText"]))
        else:
            s.add((dc.get("kind"), nu(dc.get("oldUri") or dc.get("uri")), nu(dc.get("newUri"))))
    for u, es in (x.get("changes") or {}).items():
        for e in es:
            s.add((nu(u), rg(e["range"]), e["newText"]))
    return frozenset(s)


def preparar(x):
    if erro(x):
        return "ERRO"
    if not x:
        return None
    if "range" in x:
        return (rg(x["range"]), x.get("placeholder"))
    return rg(x)


def acoes(x):
    if not x or erro(x):
        return frozenset()
    return frozenset((a.get("title"), a.get("kind")) for a in x)


def titulos(x):
    if not x or erro(x):
        return frozenset()
    return frozenset(a.get("title") for a in x)


def selecao(x):
    if not x or erro(x):
        return None
    saida = []
    for s in x:
        cad = []
        while s:
            cad.append(rg(s["range"]))
            s = s.get("parent")
        saida.append(tuple(cad))
    return tuple(saida)


def hierarquia(x):
    if not x or erro(x):
        return frozenset()
    return frozenset((i["name"], i["kind"], nu(i["uri"]), rg(i["selectionRange"])) for i in x)


def destaques(x):
    if not x or erro(x):
        return frozenset()
    return frozenset(rg(h["range"]) for h in x)


def simbolos(x):
    if not x or erro(x):
        return frozenset()
    s = set()

    def andar(l, pai):
        for i in l:
            r = i.get("selectionRange") or (i.get("location") or {}).get("range")
            s.add((i["name"], i["kind"], r["start"]["line"] if r else None))
            andar(i.get("children") or [], i["name"])
    andar(x, None)
    return frozenset(s)


def dobras(x):
    if not x or erro(x):
        return frozenset()
    return frozenset((f["startLine"], f["endLine"]) for f in x)


def dicas(x):
    if not x or erro(x):
        return frozenset()
    s = set()
    for h in x:
        l = h["label"]
        if isinstance(l, list):
            l = "".join(p["value"] for p in l)
        s.add((h["position"]["line"], h["position"]["character"], l))
    return frozenset(s)


def links(x):
    if not x or erro(x):
        return frozenset()
    return frozenset((rg(l["range"]), nu(l.get("target"))) for l in x)


def ws(x):
    if not x or erro(x):
        return frozenset()
    return frozenset((i["name"], nu((i.get("location") or {}).get("uri"))) for i in x)


def assin(x):
    if not x or erro(x) or not x.get("signatures"):
        return None
    a = x.get("activeSignature") or 0
    s = x["signatures"][min(a, len(x["signatures"]) - 1)]
    ap = s.get("activeParameter", x.get("activeParameter"))
    # O parâmetro ativo diverge de propósito (ver docs/LSP.md): só o rótulo.
    return s["label"]


def rotulo(i):
    return re.sub(r"\(.*$", "", i.get("filterText") or i["label"]).strip()


def lista_completar(x, prefixo):
    if not x or erro(x):
        return []
    itens = x["items"] if isinstance(x, dict) else x
    p = prefixo.lower()
    filtrados = [i for i in itens if rotulo(i).lower().startswith(p)]
    filtrados.sort(key=lambda i: (i.get("sortText") or i["label"]))
    return [rotulo(i) for i in filtrados]


legendas = {}
linhas = [json.loads(l) for l in open(arq, encoding="utf-8")]
for l in linhas:
    if l["metodo"] == "initialize":
        cap = (l["resultado"] or {}).get("capabilities", {})
        legendas[l["servidor"]] = (cap.get("semanticTokensProvider") or {}).get("legend")


def tokens(x, srv):
    if not x or erro(x) or not legendas.get(srv):
        return []
    d = x["data"]
    leg = legendas[srv]
    l = c = 0
    saida = []
    for i in range(0, len(d), 5):
        dl, dc, n, t, m = d[i:i + 5]
        l += dl
        c = c + dc if dl == 0 else dc
        mods = frozenset(nm for b, nm in enumerate(leg["tokenModifiers"]) if m & (1 << b))
        saida.append((l, c, n, leg["tokenTypes"][t], mods))
    return saida


NORMAS = {
    "textDocument/hover": texto_hover,
    "textDocument/definition": locais,
    "textDocument/typeDefinition": locais,
    "textDocument/implementation": locais,
    "textDocument/references": locais,
    "textDocument/documentHighlight": destaques,
    "textDocument/prepareRename": preparar,
    "textDocument/rename": edicoes,
    "textDocument/selectionRange": selecao,
    "textDocument/prepareCallHierarchy": hierarquia,
    "textDocument/prepareTypeHierarchy": hierarquia,
    "textDocument/signatureHelp": assin,
    "textDocument/documentSymbol": simbolos,
    "textDocument/foldingRange": dobras,
    "textDocument/inlayHint": dicas,
    "textDocument/documentLink": links,
    "workspace/symbol": ws,
}

estat = collections.OrderedDict()
exemplos = collections.defaultdict(list)


def conta(chave, igual, cob_dart, cob_df, ex):
    e = estat.setdefault(chave, [0, 0, 0, 0, 0.0, 0.0])
    e[0] += 1
    e[1] += igual
    e[2] += cob_dart
    e[3] += cob_df and cob_dart
    if not igual and len(exemplos[chave]) < N_EX:
        exemplos[chave].append(ex)


ms = collections.defaultdict(lambda: [[], []])
for l in linhas:
    m = l["metodo"]
    if m == "initialize" or (SO and SO not in m):
        continue
    a, b = l.get("dart"), l.get("dartforge")
    ms[m][0].append(l.get("ms_dart", 0))
    ms[m][1].append(l.get("ms_dartforge", 0))
    onde = f'{l.get("projeto")}/{l.get("arquivo")} {l.get("pos")} {l.get("nome", "")}'
    if m == "textDocument/completion":
        pre = l.get("prefixo", "")
        la, lb = lista_completar(a, pre), lista_completar(b, pre)
        alvo = l["nome"]
        tem_a, tem_b = alvo in la, alvo in lb
        conta("completion: alvo presente", tem_a == tem_b or not tem_a, tem_a, tem_b,
              (onde, "dart tem" if tem_a else "", "df tem" if tem_b else ""))
        ra = la.index(alvo) if tem_a else None
        rb = lb.index(alvo) if tem_b else None
        conta("completion: mesmo top-1", (la[:1] == lb[:1]), bool(la), bool(lb), (onde, la[:3], lb[:3]))
        sa, sb = set(la[:5]), set(lb[:5])
        conta("completion: top-5 ≥ 60% comum", len(sa & sb) >= 0.6 * max(1, len(sa)), bool(la), bool(lb),
              (onde, la[:5], lb[:5]))
        continue
    if m == "textDocument/codeAction":
        def por_especie(x, prefixo):
            if not x or erro(x):
                return frozenset()
            return frozenset(i.get("title") for i in x
                             if (i.get("kind") or "").startswith(prefixo) and not i.get("title", "").startswith("Ignore '"))
        if l.get("diagnostico") is not None:
            ta, tb = por_especie(a, "quickfix"), por_especie(b, "quickfix")
            conta("codeAction: correções (quickfix) p/ diag. do Dart", ta <= tb, bool(ta), bool(ta & tb),
                  (onde, l["diagnostico"], sorted(ta - tb), sorted(tb - ta)))
        else:
            ta, tb = por_especie(a, "refactor"), por_especie(b, "refactor")
            conta("codeAction: assistências (refactor) no cursor", ta == tb, bool(ta), bool(ta & tb),
                  (onde, sorted(ta - tb), sorted(tb - ta)))
        ta, tb = por_especie(a, "source"), por_especie(b, "source")
        conta("codeAction: ações de fonte (source)", ta == tb, bool(ta), bool(ta & tb), (onde, sorted(ta - tb), sorted(tb - ta)))
        continue
    if m == "textDocument/semanticTokens/full":
        ta, tb = tokens(a, "dart"), tokens(b, "dartforge")
        da = {(x[0], x[1], x[2]): (x[3], x[4]) for x in ta}
        db = {(x[0], x[1], x[2]): (x[3], x[4]) for x in tb}
        for k, v in da.items():
            conta("semanticTokens (por token: tipo)", db.get(k, (None,))[0] == v[0], True, k in db,
                  (onde, k, v, db.get(k)))
        continue
    if m == "textDocument/formatting":
        conta("formatting (sem edição = igual)", (not a) == (not b), bool(a), bool(b), (onde, str(a)[:80], str(b)[:80]))
        continue
    f = NORMAS.get(m)
    if not f:
        continue
    na, nb = f(a), f(b)
    nome = m.split("/")[-1]
    if m == "textDocument/hover":
        conta("hover (texto inteiro)", na == nb, na is not None, nb is not None, (onde, na, nb))
        sa, sb = assinatura_hover(a), assinatura_hover(b)
        conta("hover (assinatura)", sa == sb, sa is not None, sb is not None, (onde, sa, sb))
        continue
    conta(nome, na == nb, bool(na), bool(nb), (onde, str(na)[:300], str(nb)[:300]))

print("| Recurso | Amostras | Igual ao Dart | Dart responde | DartForge responde onde o Dart responde |")
print("|---|---|---|---|---|")
for k, (n, ig, ca, cb, _, _) in estat.items():
    print(f"| {k} | {n} | {100 * ig / n:.0f}% | {ca} | {cb}/{ca} |")
print()
print("| Método | Dart mediana ms | DartForge mediana ms |")
print("|---|---|---|")
for m, (xa, xb) in ms.items():
    xa, xb = sorted(xa), sorted(xb)
    print(f"| {m} | {xa[len(xa) // 2]:.1f} | {xb[len(xb) // 2]:.1f} |")
print()
for k, ex in exemplos.items():
    print("##", k)
    for e in ex:
        print("   ", e)
