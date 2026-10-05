#!/usr/bin/env python3
# Tabela de efeitos por extern do runtime do dartforge (heuristica textual).
# Uso, da raiz do repositorio: python scripts/analisar-efeitos-runtime.py
# Saidas (em scripts/dados/efeitos-runtime/): declaradas.tsv definidas.tsv
#   efeitos.tsv resumo.txt divergencias.tsv sementes.tsv grafo_funcoes.tsv
# O `efeitos.tsv` de la e a entrada de scripts/gerar-efeitos.py.
import os, re, sys, collections
RAIZ = sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EN = os.path.join(RAIZ, "crates/emit_native/src")
RT = os.path.join(RAIZ, "crates/runtime/src")
SAIDA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "dados", "efeitos-runtime")
os.makedirs(SAIDA, exist_ok=True)
ASPAS = chr(34)
APOSTROFO = chr(39)

# ---------------------------------------------------------------- limpeza
RE_ABRE_STR = re.compile(r'(br|rb|r|b)?(#*)"')
RE_CHAR = re.compile(r"'(\\.[^']*|[^'\\])'")

def brancos(s):
    return "".join(c if c == "\n" else " " for c in s)

def limpar(src):
    """Troca comentarios, strings e chars por espacos (mantem \\n e a string "C")."""
    out = []
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(brancos(src[i:j]))
            i = j
            continue
        if src.startswith("/*", i):
            d = 1
            j = i + 2
            while j < n and d:
                if src.startswith("/*", j):
                    d += 1
                    j += 2
                elif src.startswith("*/", j):
                    d -= 1
                    j += 2
                else:
                    j += 1
            out.append(brancos(src[i:j]))
            i = j
            continue
        m = None
        if c == ASPAS:
            m = RE_ABRE_STR.match(src, i)
        elif c in "rb" and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            m = RE_ABRE_STR.match(src, i)
            if m and not m.group(1):
                m = None
        if m:
            pre, hs = m.group(1) or "", m.group(2)
            j = m.end()
            if "r" in pre:
                fim = src.find(ASPAS + hs, j)
                fim = n if fim < 0 else fim
                k = fim + 1 + len(hs)
            else:
                while j < n and src[j] != ASPAS:
                    j += 2 if src[j] == "\\" else 1
                fim = j
                k = j + 1
            corpo = src[m.end():fim]
            if corpo == "C" and not pre:
                out.append(src[i:k])
            else:
                out.append(brancos(src[i:k]))
            i = k
            continue
        if c == APOSTROFO:
            m = RE_CHAR.match(src, i)
            if m:
                out.append(" " * (m.end() - i))
                i = m.end()
                continue
        out.append(c)
        i += 1
    return "".join(out)

def casar(txt, i):
    d = 0
    for j in range(i, len(txt)):
        if txt[j] == "{":
            d += 1
        elif txt[j] == "}":
            d -= 1
            if d == 0:
                return j
    return len(txt) - 1

# ---------------------------------------------------------------- funcoes do runtime
class F:
    __slots__ = ("nome", "arq", "linha", "impl", "metodo", "corpo", "extern", "id")

funcs = []
RE_FN = re.compile(r"\bfn\s+([A-Za-z_]\w*)")
RE_IMPL = re.compile(r"\bimpl\b([^{;]*)\{")
RE_TESTE = re.compile(r"#\[cfg\(test\)\]\s*(?:pub\s+)?mod\s+\w+\s*\{")
RE_SELF = re.compile(r"\s*(<[^>]*>)?\s*\(\s*(&\s*('\w+\s+)?)?(mut\s+)?self\b")
RE_EXTERN_C = re.compile(r'extern\s+"C"\s*$')
for arq in sorted(os.listdir(RT)):
    if not arq.endswith(".rs"):
        continue
    bruto = open(os.path.join(RT, arq), encoding="utf-8").read()
    txt = limpar(bruto)
    assert len(txt) == len(bruto), arq
    testes = [(m.start(), casar(txt, m.end() - 1)) for m in RE_TESTE.finditer(txt)]
    impls = []
    for m in RE_IMPL.finditer(txt):
        cab = re.sub(r"<[^<>]*>", "", re.sub(r"<[^<>]*>", "", m.group(1)))
        cab = cab.split(" for ")[-1].split(" where ")[0].strip()
        tipo = re.findall(r"[A-Za-z_]\w*", cab)
        impls.append((m.start(), casar(txt, m.end() - 1), tipo[-1] if tipo else "?"))
    for m in RE_FN.finditer(txt):
        p = m.start()
        if any(a <= p <= b for a, b in testes):
            continue
        j = m.end()
        d = 0
        ab = -1
        while j < len(txt):
            ch = txt[j]
            if ch in "([":
                d += 1
            elif ch in ")]":
                d -= 1
            elif ch == "{" and d == 0:
                ab = j
                break
            elif ch == ";" and d == 0:
                break
            j += 1
        if ab < 0:
            continue
        fe = casar(txt, ab)
        f = F()
        f.nome = m.group(1)
        f.arq = arq
        f.linha = txt.count("\n", 0, p) + 1
        dono = [t for a, b, t in impls if a <= p <= b]
        f.impl = dono[-1] if dono else ""
        f.metodo = bool(RE_SELF.match(txt[m.end():ab]))
        f.corpo = txt[p:fe + 1]
        ini_linha = txt.rfind("\n", 0, p) + 1
        antes = bruto[max(0, ini_linha - 400):ini_linha]
        f.extern = bool(RE_EXTERN_C.search(txt[ini_linha:p].rstrip())) and "no_mangle" in antes.split("}\n")[-1]
        f.id = len(funcs)
        funcs.append(f)

livres = collections.defaultdict(list)
metodos = collections.defaultdict(list)
assoc = collections.defaultdict(list)
for f in funcs:
    (metodos if f.metodo else livres)[f.nome].append(f)
    if f.impl:
        assoc[(f.impl, f.nome)].append(f)

# nomes de metodo que colidem com a std e nao sao resolvidos por nome
IGNORAR_METODOS = {"collect"}
RE_CHAMADA = re.compile(r"(\.\s*|(?:([A-Za-z_]\w*)\s*::\s*))?\b([A-Za-z_]\w*)\s*(?:::\s*<[^>]*>\s*)?\(")
RE_VALOR = re.compile(r"[(,]\s*([a-z_]\w*)\s*(?=[),])")
PALAVRAS = {"if", "while", "match", "for", "fn", "return", "loop", "Some", "Ok", "Err", "None", "in", "as", "let", "move", "unsafe", "else"}

def chamadas(f):
    alvo = set()
    corpo = f.corpo[f.corpo.find("{"):]
    for m in RE_CHAMADA.finditer(corpo):
        pre, tipo, nome = m.group(1), m.group(2), m.group(3)
        if nome in PALAVRAS:
            continue
        if pre and pre.strip().startswith("."):
            if nome not in IGNORAR_METODOS:
                alvo.update(g.id for g in metodos.get(nome, ()))
        elif tipo:
            if tipo == "Self":
                tipo = f.impl
            if tipo[:1].isupper():
                alvo.update(g.id for g in assoc.get((tipo, nome), ()))
            else:
                alvo.update(g.id for g in livres.get(nome, ()) if not g.impl)
        else:
            alvo.update(g.id for g in livres.get(nome, ()) if not g.impl)
    for m in RE_VALOR.finditer(corpo):
        nome = m.group(1)
        if nome in livres and re.search(r"\b(let|mut)\s+" + nome + r"\b|\b" + nome + r"\s*:", f.corpo) is None:
            alvo.update(g.id for g in livres[nome] if not g.impl)
    alvo.discard(f.id)
    return alvo

grafo = {f.id: chamadas(f) for f in funcs}

# ---------------------------------------------------------------- sementes
SEM_COLETA = {"alocar_bloco", "alocar_lento", "antes_de_alocar", "coletar", "coletar_automatico"}
RE_COLETA_DIRETA = re.compile(r"borrow_mut\(\)\s*\.\s*collect\(\)")
SEM_LANCA = {"definir_excecao"}
RE_PONTEIRO = re.compile(r'extern\s+"C"\s+fn\s*\(')
RE_AJUDANTE = re.compile(r"\b(ajudante|ajudante_de_io|ajudante_de_isolado)\s*\(|\bPARA_TEXTO\b")
DECLARA_AJUDANTE = {"ajudante", "ajudante_de_io", "ajudante_de_isolado", "dartforge_registrar_ajudante", "dartforge_iniciar"}
# ponteiros `extern "C" fn` que sao codigo C (finalizadores nativos, APIs do SO), nao Dart
PONTEIRO_C = {("heap.rs", "encerrar_finalizadores"), ("heap.rs", "coletar")}
ARQ_C = {"io_observador.rs", "io_windows_eventos.rs", "ffi_api_nativa.rs"}
# ponteiro de funcao GERADA que so devolve uma tabela (nao e Dart do usuario)
RE_TABELA = re.compile(r'extern\s+"C"\s+fn\s*\(\s*\)\s*->\s*\*const\s+i64')
coleta, lanca, dart = {}, {}, {}
# niveis de "roda Dart": usuario (codigo Dart arbitrario) > sdk (funcao da sobreposicao do SDK
# registrada por dartforge_registrar_ajudante: construtor de erro/objeto) > tabela (getter gerado)
AJUDANTE_USUARIO = {"invocar_no_such_method", "refazer_indices_copiados", "finalizar_programa", "relatar_erro_nao_tratado", "rodar_isolado"}
# guardam/enfileiram o ponteiro sem chamar, ou chamam codigo C do proprio runtime
PONTEIRO_NAO_CHAMADO = {"dartforge_registrar_isolados", "dartforge_pedir_no_ponto_seguro", "parar_neste_isolado", "dartforge_iniciar"}
sementes = []
for f in funcs:
    if f.nome in SEM_COLETA and f.arq == "heap.rs":
        coleta[f.id] = "SEMENTE heap::" + f.nome
    elif RE_COLETA_DIRETA.search(f.corpo):
        coleta[f.id] = "SEMENTE Heap::collect()"
    if f.nome in SEM_LANCA:
        lanca[f.id] = "SEMENTE " + f.nome
    m = RE_AJUDANTE.search(f.corpo)
    if m and f.nome not in DECLARA_AJUDANTE:
        nivel = "usuario" if f.nome in AJUDANTE_USUARIO else "sdk"
        dart[f.id] = "SEMENTE[%s] %s (funcao Dart registrada)" % (nivel, m.group(0).rstrip("(").strip())
    elif RE_PONTEIRO.search(f.corpo) and (f.arq, f.nome) not in PONTEIRO_C and f.arq not in ARQ_C and f.nome not in PONTEIRO_NAO_CHAMADO:
        so_tabela = all(RE_TABELA.match(f.corpo, x.start()) for x in RE_PONTEIRO.finditer(f.corpo))
        if so_tabela:
            dart[f.id] = "SEMENTE[tabela] ponteiro fn() -> *const i64 (tabela gerada, nao Dart)"
        else:
            dart[f.id] = "SEMENTE[usuario] ponteiro extern C fn (transmute/parametro)"
    if f.id in dart:
        sementes.append((f.arq, f.linha, f.nome, dart[f.id]))

def propagar(efeito, fila0=None):
    inv = collections.defaultdict(set)
    for a, bs in grafo.items():
        for b in bs:
            inv[b].add(a)
    fila = collections.deque(sorted(efeito))
    while fila:
        b = fila.popleft()
        for a in sorted(inv[b]):
            if a not in efeito:
                efeito[a] = "via " + funcs[b].nome + " <- " + (efeito[b] if efeito[b].startswith("SEMENTE") else efeito[b][4:])
                fila.append(a)

# o nivel mais forte vence: propaga usuario, depois sdk, depois tabela
todas = dict(dart)
# uma semente local de nivel menor nao pode rebaixar: refaz na ordem certa
dart.clear()
for nivel in ("[usuario]", "[sdk]", "[tabela]"):
    for k, v in todas.items():
        if nivel in v and k not in dart:
            dart[k] = v
    propagar(dart)
propagar(coleta)
propagar(lanca)

def nivel_de(d):
    for n in ("usuario", "sdk", "tabela"):
        if "[" + n + "]" in d:
            return n
    return ""

# ---------------------------------------------------------------- declaradas pelo emissor
decl = {}
ext = open(os.path.join(EN, "llvm/externs.rs"), encoding="utf-8").read()
RE_DECL = re.compile(r'decl:\s*"declare [^@"]*@(dartforge_\w+)\(([^"]*)"\s*,(.*?)efeitos:\s*(ALOCA_SEM_LANCAR|CONSERVADOR|Efeitos\s*\{[^}]*\})', re.S)
for m in RE_DECL.finditer(ext):
    e = m.group(4)
    if e == "CONSERVADOR":
        ef = (1, 1, 0)
    elif e == "ALOCA_SEM_LANCAR":
        ef = (1, 0, 0)
    else:
        ef = tuple(int(re.search(k + r":\s*(true|false)", e).group(1) == "true") for k in ("aloca", "lanca", "chama_dart"))
    decl[m.group(1)] = ("externs.rs:%d" % (ext.count("\n", 0, m.start()) + 1), ef)
nat = open(os.path.join(EN, "nativos.rs"), encoding="utf-8").read()
for m in re.finditer(r'\bruntime\("(\w+)"\)', nat):
    decl.setdefault("dartforge_nativo_" + m.group(1), ("nativos.rs:%d" % (nat.count("\n", 0, m.start()) + 1), (1, 1, 0)))
prefixos = {}
for base, _, arqs in os.walk(EN):
    for a in sorted(arqs):
        if not a.endswith(".rs"):
            continue
        cam = os.path.join(base, a)
        t = open(cam, encoding="utf-8").read()
        rel = os.path.relpath(cam, EN).replace("\\", "/")
        for m in re.finditer(r"dartforge_\w+", t):
            nome = m.group(0)
            onde = "%s:%d" % (rel, t.count("\n", 0, m.start()) + 1)
            if nome.endswith("_"):
                prefixos.setdefault(nome, onde)
            else:
                decl.setdefault(nome, (onde, None))

# ---------------------------------------------------------------- saidas
defs = {}
for f in funcs:
    if f.extern and f.nome.startswith("dartforge_"):
        defs.setdefault(f.nome, []).append(f)

def uniao(nome, ef):
    ms = [ef[f.id] for f in defs[nome] if f.id in ef]
    for n in ("[usuario]", "[sdk]", "[tabela]"):
        for x in ms:
            if n in x:
                return x
    return (ms or [""])[0]

def w(nome, ls):
    open(os.path.join(SAIDA, nome), "w", encoding="utf-8", newline="\n").write("\n".join(ls) + "\n")

w("definidas.tsv", ["%s\t%s" % (n, ",".join("%s:%d" % (f.arq, f.linha) for f in fs)) for n, fs in sorted(defs.items())])
w("declaradas.tsv", ["%s\t%s\t%s" % (n, o, "" if e is None else "aloca=%d lanca=%d chama_dart=%d" % e) for n, (o, e) in sorted(decl.items())])
usadas = {n for n in defs if n in decl or any(n.startswith(p) for p in prefixos)}
classes = collections.Counter()
ls = ["nome\tarquivo:linha\tcoleta\tlanca\troda_dart\tchamada_pelo_emissor\tclasse\tmotivo"]
tabela = {}
for n in sorted(defs):
    c, l, d = uniao(n, coleta), uniao(n, lanca), uniao(n, dart)
    nv = nivel_de(d)
    if nv in ("usuario", "sdk"):
        cl = "roda_dart_" + nv
    elif c and l:
        cl = "coleta+lanca"
    elif c:
        cl = "so_coleta"
    elif l:
        cl = "so_lanca"
    else:
        cl = "folha_pura"
    if nv == "tabela":
        cl += "+tabela"
    forte = nv in ("usuario", "sdk")
    tabela[n] = (bool(c) or forte, bool(l) or forte, forte, cl)
    if n in usadas:
        classes[cl] += 1
    motivo = "; ".join(x for x in ("coleta: " + c if c else "", "lanca: " + l if l else "", "dart: " + d if d else "") if x)[:700]
    ls.append("	".join([n, ",".join("%s:%d" % (f.arq, f.linha) for f in defs[n]), str(int(tabela[n][0])), str(int(tabela[n][1])), nv or "0", str(int(n in usadas)), cl, motivo]))
w("efeitos.tsv", ls)
w("sementes.tsv", ["%s:%d\t%s\t%s" % s for s in sorted(sementes)])
w("grafo_funcoes.tsv", ["%s:%d\t%s%s\t%s" % (f.arq, f.linha, (f.impl + "::") if f.impl else "", f.nome, " ".join(sorted({funcs[g].nome for g in grafo[f.id]}))) for f in funcs])
div = ["nome\temissor(aloca,lanca,dart)\tanalise(coleta,lanca,dart)\ttipo\tmotivo"]
for n, (o, e) in sorted(decl.items()):
    if e is None or n not in tabela or not o.startswith("externs.rs"):
        continue
    c, l, d, cl = tabela[n]
    tipos = []
    if not e[0] and not e[2] and (c or d):
        tipos.append("EMISSOR_NAO_COLETA_MAS_ANALISE_COLETA")
    if not e[1] and (l or d):
        tipos.append("EMISSOR_NAO_LANCA_MAS_ANALISE_LANCA")
    if e[0] and not c and not d:
        tipos.append("emissor_conservador_coleta")
    if e[1] and not l and not d:
        tipos.append("emissor_conservador_lanca")
    for t in tipos:
        div.append("%s\t%d,%d,%d\t%d,%d,%d\t%s\t%s" % (n, e[0], e[1], e[2], c, l, d, t, "; ".join(x for x in (uniao(n, coleta), uniao(n, lanca), uniao(n, dart)) if x)[:500]))
w("divergencias.tsv", div)
d_ext = {n for n, (o, e) in decl.items() if o.startswith("externs.rs") and e is not None}
d_nat = {n for n, (o, e) in decl.items() if o.startswith("nativos.rs")}
res = []
res.append("funcoes do runtime analisadas (fora de testes): %d; arestas: %d" % (len(funcs), sum(len(v) for v in grafo.values())))
res.append("externs definidas no runtime: %d nomes (%d definicoes)" % (len(defs), sum(len(v) for v in defs.values())))
res.append("declaradas pelo emissor: %d (EXTERNS: %d; natives Runtime de nativos.rs fora de EXTERNS: %d; outros nomes literais em EN/: %d); prefixos de format!: %d" % (len(decl), len(d_ext), len(d_nat), len(decl) - len(d_ext) - len(d_nat), len(prefixos)))
res.append("intersecao (definidas e citadas pelo emissor, contando prefixos): %d" % len(usadas))
so_decl = sorted(n for n in decl if n not in defs)
so_def = sorted(n for n in defs if n not in usadas)
res.append("declaradas sem definicao no runtime: %d -> %s" % (len(so_decl), " ".join("%s(%s)" % (n, decl[n][0]) for n in so_decl)))
res.append("definidas e nao citadas pelo emissor: %d -> %s" % (len(so_def), " ".join(so_def)))
res.append("prefixos: " + " ".join("%s(%s)" % kv for kv in sorted(prefixos.items())))
res.append("classes (so as citadas pelo emissor): " + ", ".join("%s=%d" % kv for kv in sorted(classes.items())))
tot = collections.Counter(v[3] for v in tabela.values())
res.append("classes (todas as definidas): " + ", ".join("%s=%d" % kv for kv in sorted(tot.items())))
for cl in ("folha_pura", "folha_pura+tabela", "so_lanca", "so_coleta", "so_coleta+tabela", "coleta+lanca", "coleta+lanca+tabela"):
    res.append("%s citadas pelo emissor: %s" % (cl.upper(), " ".join(sorted(n for n in usadas if tabela[n][3] == cl))))
# limitacoes: metodos resolvidos so pelo nome que carregam efeito, e impls de trait com efeito
porm = collections.defaultdict(set)
for f in funcs:
    if f.metodo:
        e = "".join(x for x, t in (("C", coleta), ("L", lanca), ("D", dart)) if f.id in t)
        if e:
            porm[f.nome].add("%s::%s[%s]" % (f.impl, f.nome, e))
res.append("metodos (resolvidos so pelo nome) com efeito: " + " ".join(sorted(x for v in porm.values() for x in v)))
homonimos = sorted(n for n, fs in metodos.items() if len({g.impl for g in fs}) > 1)
res.append("nomes de metodo com mais de um tipo dono (uniao): " + " ".join(homonimos))
res.append("divergencias: %d linhas (divergencias.tsv)" % (len(div) - 1))
w("resumo.txt", res)
print("\n".join(res))
