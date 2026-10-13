# Gera as tabelas de codigos e de regras de lint do DartForge a partir da
# fonte oficial 3.6.2 (so leitura). Saidas: arquivos *_g.rs novos, da frente
# INFRA-FASES. Rodar da raiz do repositorio: python scripts/generate-analysis-tables.py
import os
import re
import sys

AN = r"E:\references\dart-sdk-3.6.2\pkg\analyzer\lib"
FE = r"E:\references\dart-sdk-3.6.2\pkg\_fe_analyzer_shared\lib"
LI = r"E:\dftemp\analise\spec-infra\sdk362\pkg\linter\lib\src"
LINTS = r"C:\Users\pmro\AppData\Local\Pub\Cache\hosted\pub.dev\lints-5.1.1\lib"
SAIDA = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "crates", "analysis", "src")


def ler(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


def pular_brancos(s, i):
    n = len(s)
    while i < n:
        if s[i].isspace():
            i += 1
        elif s.startswith("//", i):
            j = s.find("\n", i)
            i = n if j < 0 else j + 1
        elif s.startswith("/*", i):
            j = s.find("*/", i)
            i = n if j < 0 else j + 2
        else:
            break
    return i


def ler_string(s, i):
    """Le um literal de string Dart em s[i:]; devolve (valor, fim) ou None."""
    raw = False
    if s.startswith("r'", i) or s.startswith('r"', i):
        raw = True
        i += 1
    if i >= len(s) or s[i] not in "'\"":
        return None
    q = s[i]
    if s.startswith(q * 3, i):
        fim = s.find(q * 3, i + 3)
        return s[i + 3:fim], fim + 3
    i += 1
    out = []
    while s[i] != q:
        c = s[i]
        if c == "\\" and not raw:
            d = s[i + 1]
            if d == "n":
                out.append("\n")
            elif d == "t":
                out.append("\t")
            elif d == "r":
                out.append("\r")
            else:
                out.append(d)
            i += 2
        else:
            out.append(c)
            i += 1
    return "".join(out), i + 1


def ler_strings(s, i):
    """Literais adjacentes; devolve (valor, fim) ou None."""
    i = pular_brancos(s, i)
    r = ler_string(s, i)
    if r is None:
        return None
    partes = [r[0]]
    i = r[1]
    while True:
        j = pular_brancos(s, i)
        r = ler_string(s, j)
        if r is None:
            return "".join(partes), i
        partes.append(r[0])
        i = r[1]


def fechar_parenteses(s, i):
    """s[i] == '('; devolve o indice do ')' correspondente, pulando strings."""
    prof = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c in "'\"" or (c == "r" and i + 1 < n and s[i + 1] in "'\"" and not (s[i - 1].isalnum() or s[i - 1] == "_")):
            r = ler_string(s, i)
            i = r[1]
            continue
        if s.startswith("//", i):
            j = s.find("\n", i)
            i = n if j < 0 else j + 1
            continue
        if c == "(":
            prof += 1
        elif c == ")":
            prof -= 1
            if prof == 0:
                return i
        i += 1
    raise ValueError("parenteses")


def argumentos(corpo):
    """Divide o texto entre parenteses em argumentos de topo."""
    args = []
    prof = 0
    i = 0
    ini = 0
    n = len(corpo)
    while i < n:
        c = corpo[i]
        if c in "'\"" or (c == "r" and i + 1 < n and corpo[i + 1] in "'\"" and (i == 0 or not (corpo[i - 1].isalnum() or corpo[i - 1] == "_"))):
            r = ler_string(corpo, i)
            i = r[1]
            continue
        if corpo.startswith("//", i):
            j = corpo.find("\n", i)
            i = n if j < 0 else j + 1
            continue
        if c in "([{":
            prof += 1
        elif c in ")]}":
            prof -= 1
        elif c == "," and prof == 0:
            args.append(corpo[ini:i])
            ini = i + 1
        i += 1
    if corpo[ini:].strip():
        args.append(corpo[ini:])
    return args


RE_CONST = re.compile(r"static\s+const\s+(\w+)\s+(\w+)\s*=\s*(?:const\s+)?(\w+)\s*\(")


def codigos_de(texto):
    """Lista de dict(classe, const, nome, mensagem, correcao, documentado, unico)."""
    out = []
    for m in RE_CONST.finditer(texto):
        ab = m.end() - 1
        try:
            fe = fechar_parenteses(texto, ab)
        except Exception:
            continue
        args = argumentos(texto[ab + 1:fe])
        if len(args) < 2:
            continue
        a0 = args[0].strip()
        r0 = ler_strings(a0, 0)
        if r0 is not None:
            nome = r0[0]
        elif a0.startswith("LintNames."):
            nome = a0[len("LintNames."):]
        else:
            continue
        r1 = ler_strings(args[1], 0)
        if r1 is None:
            continue
        d = dict(classe=m.group(3), const=m.group(2), nome=nome, mensagem=r1[0], correcao=None, documentado=False, unico=None)
        for a in args[2:]:
            a = a.strip()
            mm = re.match(r"(\w+)\s*:", a)
            if not mm:
                continue
            chave = mm.group(1)
            resto = a[mm.end():]
            if chave == "correctionMessage":
                r = ler_strings(resto, 0)
                if r:
                    d["correcao"] = r[0]
            elif chave == "hasPublishedDocs":
                d["documentado"] = resto.strip() == "true"
            elif chave == "uniqueName":
                r = ler_strings(resto, 0)
                if r:
                    d["unico"] = r[0]
        out.append(d)
    return out


def rs(s):
    """Literal de string Rust."""
    if s is None:
        return "None"
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + '"'


def opt(s):
    return "None" if s is None else "Some(" + rs(s) + ")"


def escrever(caminho, texto):
    os.makedirs(os.path.dirname(caminho), exist_ok=True)
    with open(caminho, "w", encoding="utf-8", newline="\n") as f:
        f.write(texto)
    print("escrito", caminho, len(texto))


# ---------------------------------------------------------------------------
# 1. Codigos dos arquivos nao-Dart
# ---------------------------------------------------------------------------
CLASSES_ND = {
    "AnalysisOptionsErrorCode": ("Error", "CompileTimeError"),
    "AnalysisOptionsHintCode": ("Info", "Hint"),
    "AnalysisOptionsWarningCode": ("Warning", "StaticWarning"),
    "PubspecWarningCode": ("Warning", "StaticWarning"),
    "ManifestWarningCode": ("Warning", "StaticWarning"),
}
MODULOS_ND = {
    "AnalysisOptionsErrorCode": "opcoes",
    "AnalysisOptionsHintCode": "opcoes",
    "AnalysisOptionsWarningCode": "opcoes",
    "PubspecWarningCode": "pubspec",
    "ManifestWarningCode": "manifesto",
}
fontes_nd = [
    os.path.join(AN, "src", "analysis_options", "error", "option_codes.g.dart"),
    os.path.join(AN, "src", "pubspec", "pubspec_warning_code.g.dart"),
    os.path.join(AN, "src", "manifest", "manifest_warning_code.g.dart"),
]
nd = []
for p in fontes_nd:
    t = ler(p)
    # confere severidade/tipo declarados na classe
    for cls, (sev, tipo) in CLASSES_ND.items():
        mm = re.search(r"class " + cls + r" extends ErrorCode \{(.*?)\n\}", t, re.S)
        if mm:
            corpo = mm.group(1)
            s = re.search(r"errorSeverity => ErrorSeverity\.(\w+)", corpo).group(1)
            ty = re.search(r"get type => ErrorType\.(\w+)", corpo).group(1)
            esperado = {"Error": "ERROR", "Info": "INFO", "Warning": "WARNING"}[sev]
            et = {"CompileTimeError": "COMPILE_TIME_ERROR", "Hint": "HINT", "StaticWarning": "STATIC_WARNING"}[tipo]
            assert s == esperado and ty == et, (cls, s, ty)
    nd.extend(c for c in codigos_de(t) if c["classe"] in CLASSES_ND)
print("nao-dart:", len(nd))
assert len(nd) == 53, len(nd)

linhas = [
    "// GENERATED by `scripts/generate-analysis-tables.py` (INFRA-FASES front). DO NOT EDIT.",
    "// Source: the SDK 3.6.2 analyzer — `analysis_options/error/option_codes.g.dart`,",
    "// `pubspec/pubspec_warning_code.g.dart`, `manifest/manifest_warning_code.g.dart`.",
    "// TODO(catalog): these 53 codes are not in `crates/diagnostics/src/codes_g.rs`;",
    "// once they are, `Report` converts to a diagnostic with `Diagnostic::with_code`.",
    "#![allow(missing_docs)]",
    "",
    "use super::NonDartCode;",
    "use dartforge_diagnostics::{Severity, ErrorType};",
    "",
]
for mod in ["options", "pubspec", "manifest"]:
    linhas.append("pub mod %s {" % mod)
    linhas.append("    use super::*;")
    for c in nd:
        if MODULOS_ND[c["classe"]] != mod:
            continue
        sev, tipo = CLASSES_ND[c["classe"]]
        linhas.append(
            "    pub static %s: NonDartCode = NonDartCode { name: %s, unique: %s, message: %s, correction: %s, error_type: ErrorType::%s, severity: Severity::%s, documented: %s };"
            % (c["const"], rs(c["nome"].lower()), rs(c["classe"] + "." + (c["unico"] or c["nome"])), rs(c["mensagem"]), opt(c["correcao"]), tipo, sev, "true" if c["documentado"] else "false")
        )
    linhas.append("}")
    linhas.append("")
linhas.append("/// All the codes of non-Dart files, in source order.")
linhas.append("pub static ALL: [&NonDartCode; %d] = [" % len(nd))
for c in nd:
    linhas.append("    &%s::%s," % (MODULOS_ND[c["classe"]], c["const"]))
linhas.append("];")
escrever(os.path.join(SAIDA, "non_dart", "codes_g.rs"), "\n".join(linhas) + "\n")

# ---------------------------------------------------------------------------
# 2. Nomes de todos os errorCodeValues (para `unrecognized_error_code`)
# ---------------------------------------------------------------------------
todas = {}
for raiz in [AN, FE]:
    for d, _, fs in os.walk(raiz):
        for f in fs:
            if f.endswith(".dart"):
                try:
                    t = ler(os.path.join(d, f))
                except Exception:
                    continue
                if "static const" not in t:
                    continue
                for c in codigos_de(t):
                    todas.setdefault((c["classe"], c["const"]), c["nome"])
valores = re.findall(r"^\s+(\w+)\.(\w+),\s*$", ler(os.path.join(AN, "src", "error", "error_code_values.g.dart")), re.M)
nomes = set()
faltando = []
for cls, const in valores:
    n = todas.get((cls, const))
    if n is None:
        faltando.append((cls, const))
        # o nome padrao e a propria constante
        n = const
    nomes.add(n.upper())
print("errorCodeValues:", len(valores), "nomes distintos:", len(nomes), "sem definicao achada:", len(faltando), faltando[:10])
nomes = sorted(nomes)
linhas = [
    "// GENERATED by `scripts/generate-analysis-tables.py` (INFRA-FASES front). DO NOT EDIT.",
    "// Source: `analyzer/lib/src/error/error_code_values.g.dart` of SDK 3.6.2: the `name`",
    "// (upper case) of each `ErrorCode` of `errorCodeValues`, sorted for binary search.",
    "",
    "/// `ErrorCode.name.toUpperCase()` of `errorCodeValues` (%d constants, %d names)." % (len(valores), len(nomes)),
    "pub static CODE_NAMES: [&str; %d] = [" % len(nomes),
]
for n in nomes:
    linhas.append("    %s," % rs(n))
linhas.append("];")
escrever(os.path.join(SAIDA, "non_dart", "names_g.rs"), "\n".join(linhas) + "\n")

# ---------------------------------------------------------------------------
# 3. Regras de lint: nome, estado, incompativeis, conjunto do package:lints
# ---------------------------------------------------------------------------
VERSOES = {"dart2_12": (2, 12, 0), "dart3": (3, 0, 0), "dart3_3": (3, 3, 0)}
regras_por_classe = {}
for sub in ["rules", os.path.join("rules", "pub")]:
    pasta = os.path.join(LI, sub)
    for f in sorted(os.listdir(pasta)):
        if not f.endswith(".dart"):
            continue
        t = ler(os.path.join(pasta, f))
        for mm in re.finditer(r"class (\w+) extends (?:LintRule|PubspecLintRule|\w*LintRule\w*)\b", t):
            cls = mm.group(1)
            trecho = t[mm.end():]
            prox = re.search(r"\nclass \w+", trecho)
            if prox:
                trecho = trecho[:prox.start()]
            mn = re.search(r"name:\s*(?:LintNames\s*\.\s*(\w+)|'([^']+)')", trecho)
            if mn:
                nome = mn.group(1) or mn.group(2)
            elif re.search(r"name:\s*code\.name", trecho):
                nome = f[:-5]
            else:
                continue
            estado, desde = "Stable", None
            ms = re.search(r"state:\s*State\.(\w+)\(([^)]*(?:\([^)]*\))?[^)]*)\)", trecho)
            if ms:
                estado = {"stable": "Stable", "experimental": "Experimental", "removed": "Removed", "internal": "Internal", "deprecated": "Deprecated"}[ms.group(1)]
                arg = ms.group(2)
                mv = re.search(r"since:\s*(\w+)(?:\((\d+),\s*(\d+),\s*(\d+)\)?)?", arg)
                if mv:
                    if mv.group(2):
                        desde = (int(mv.group(2)), int(mv.group(3)), int(mv.group(4)))
                    else:
                        desde = VERSOES[mv.group(1)]
            inc = []
            mi = re.search(r"get incompatibleRules\s*=>\s*const\s*\[(.*?)\]", trecho, re.S)
            if mi:
                inc = re.findall(r"LintNames\.(\w+)", mi.group(1))
            regras_por_classe[cls] = dict(nome=nome, estado=estado, desde=desde, inc=inc, arquivo=(sub + "/" + f).replace("\\", "/"))
ordem = re.findall(r"\.\.register(?:LintRule)?\((\w+)\(\)\)", ler(os.path.join(LI, "rules.dart")))
print("registradas:", len(ordem), "classes de regra achadas:", len(regras_por_classe))
faltam = [c for c in ordem if c not in regras_por_classe]
print("sem classe:", faltam)


def conjunto_de(arq):
    t = ler(os.path.join(LINTS, arq))
    return re.findall(r"^\s+-\s+(\w+)\s*$", t, re.M)


core = conjunto_de("core.yaml")
recomendadas = conjunto_de("recommended.yaml")
print("core:", len(core), "recommended:", len(recomendadas))
linhas = [
    "// GENERATED by `scripts/generate-analysis-tables.py` (INFRA-FASES front). DO NOT EDIT.",
    "// Source: `linter/lib/src/rules.dart` (registration order) and `linter/lib/src/rules/**.dart`",
    "// (name, `state:`, `incompatibleRules`) of SDK 3.6.2; sets of `package:lints` 5.1.1.",
    "",
    "use super::{RuleSet, RuleState, RuleInfo};",
    "",
    "/// The %d rules registered by `registerLintRules()`, in registration order." % len(ordem),
    "pub static RULES: [RuleInfo; %d] = [" % len(ordem),
]
for cls in ordem:
    r = regras_por_classe[cls]
    conj = "Core" if r["nome"] in core else ("Recommended" if r["nome"] in recomendadas else "Unlisted")
    desde = "None" if r["desde"] is None else "Some((%d, %d, %d))" % r["desde"]
    linhas.append(
        "    RuleInfo { name: %s, state: RuleState::%s, since: %s, incompatible: &[%s], set: RuleSet::%s, file: %s },"
        % (rs(r["nome"]), r["estado"], desde, ", ".join(rs(x) for x in r["inc"]), conj, rs(r["arquivo"]))
    )
linhas.append("];")
escrever(os.path.join(SAIDA, "lints", "table_g.rs"), "\n".join(linhas) + "\n")
nomes_reg = set(regras_por_classe[c]["nome"] for c in ordem)
print("core fora do registro:", [x for x in core if x not in nomes_reg], "recommended fora:", [x for x in recomendadas if x not in nomes_reg])

# ---------------------------------------------------------------------------
# 4. LinterLintCode
# ---------------------------------------------------------------------------
lc = [c for c in codigos_de(ler(os.path.join(LI, "linter_lint_codes.dart"))) if c["classe"] == "LinterLintCode"]
print("LinterLintCode:", len(lc))
linhas = [
    "// GENERATED by `scripts/generate-analysis-tables.py` (INFRA-FASES front). DO NOT EDIT.",
    "// Source: `linter/lib/src/linter_lint_codes.dart` of SDK 3.6.2 (LinterLintCode).",
    "// TODO(catalog): the `LintCode`s are not in `crates/diagnostics/src/codes_g.rs`.",
    "#![allow(missing_docs)]",
    "",
    "use super::LintCode;",
    "",
]
vistos = set()
for c in lc:
    k = c["const"].upper()
    assert k not in vistos, k
    vistos.add(k)
    linhas.append(
        "pub static %s: LintCode = LintCode { name: %s, unique: %s, message: %s, correction: %s, documented: %s };"
        % (k, rs(c["nome"]), rs(c["unico"] or c["nome"]), rs(c["mensagem"]), opt(c["correcao"]), "true" if c["documentado"] else "false")
    )
linhas.append("")
linhas.append("/// All the `LinterLintCode`s, in source order.")
linhas.append("pub static ALL: [&LintCode; %d] = [" % len(lc))
for c in lc:
    linhas.append("    &%s," % c["const"].upper())
linhas.append("];")
escrever(os.path.join(SAIDA, "lints", "codes_g.rs"), "\n".join(linhas) + "\n")
