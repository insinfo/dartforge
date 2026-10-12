#!/usr/bin/env python3
"""Do que é feito um arquivo JS de produção, por categoria de token.

    python3 scripts/medir-js-producao-tokens.py saida.js [--top 30]

Lê o JS (minificado ou não) com um tokenizador simples (o mesmo critério de
regex × divisão de `crates/emit_js_production/src/minificar.rs`) e imprime:

* bytes por categoria: nomes de propriedade (`.x` e chaves `x:`), strings,
  *template literals*, identificadores, palavras-chave, números, pontuação;
* os nomes de propriedade e as strings que mais custam (bytes = tamanho ×
  ocorrências).

Serve para comparar a nossa saída com a do `dart2js` e decidir o que atacar
(`docs/JS-PRODUCAO-TAMANHO.md`). Não depende de node.
"""
import collections
import re
import sys

PALAVRAS = set(
    "break case catch class const continue debugger default delete do else export extends false finally for function if import in "
    "instanceof let new null return super switch this throw true try typeof var void while with yield await async of static get set "
    "undefined".split()
)
ANTES_DE_EXPRESSAO = {"return", "typeof", "instanceof", "in", "of", "new", "delete", "void", "throw", "case", "do", "else", "yield", "await"}
PALAVRA = re.compile(r"[A-Za-z0-9_$\x80-￿\\]+")
NUMERO = re.compile(r"(0[xX][0-9a-fA-F_]+n?|(\d[\d_]*\.?[\d_]*|\.\d[\d_]*)([eE][+-]?\d+)?n?)")


def tokens(s):
    i, n = 0, len(s)
    ultimo = None  # (espécie, texto)
    while i < n:
        c = s[i]
        if c in " \t\r\n\v\f":
            i += 1
            continue
        if s.startswith("//", i):
            j = s.find("\n", i)
            i = n if j < 0 else j
            continue
        if s.startswith("/*", i):
            j = s.find("*/", i + 2)
            i = n if j < 0 else j + 2
            continue
        if c in "\"'":
            j = i + 1
            while j < n and s[j] != c:
                j += 2 if s[j] == "\\" else 1
            yield ("string", s[i : j + 1])
            ultimo = ("lit", None)
            i = j + 1
            continue
        if c == "`":
            j = i + 1
            prof = 0
            while j < n:
                if s[j] == "\\":
                    j += 2
                    continue
                if prof == 0 and s[j] == "`":
                    break
                if s.startswith("${", j):
                    prof += 1
                    j += 2
                    continue
                if prof and s[j] == "}":
                    prof -= 1
                j += 1
            yield ("template", s[i : j + 1])
            ultimo = ("lit", None)
            i = j + 1
            continue
        if c == "/":
            pode = ultimo is None or (ultimo[0] == "pont" and ultimo[1] not in ")]") or (ultimo[0] == "palavra" and ultimo[1] in ANTES_DE_EXPRESSAO)
            if pode:
                j = i + 1
                classe = False
                ok = False
                while j < n and s[j] != "\n":
                    if s[j] == "\\":
                        j += 2
                        continue
                    if s[j] == "[":
                        classe = True
                    elif s[j] == "]":
                        classe = False
                    elif s[j] == "/" and not classe:
                        ok = True
                        break
                    j += 1
                if ok:
                    j += 1
                    while j < n and s[j].isalpha():
                        j += 1
                    yield ("regex", s[i:j])
                    ultimo = ("lit", None)
                    i = j
                    continue
        if c.isdigit() or (c == "." and i + 1 < n and s[i + 1].isdigit()):
            m = NUMERO.match(s, i)
            yield ("numero", m.group(0))
            ultimo = ("num", m.group(0))
            i = m.end()
            continue
        m = PALAVRA.match(s, i)
        if m:
            w = m.group(0)
            if ultimo is not None and ultimo[0] == "pont" and ultimo[1] in (".", "?."):
                esp = "propriedade"
            elif w in PALAVRAS:
                esp = "palavra-chave"
            else:
                # chave de objeto literal: `x:` depois de `{` ou `,`
                k = m.end()
                while k < n and s[k] in " \t\n":
                    k += 1
                if k < n and s[k] == ":" and ultimo is not None and ultimo[0] == "pont" and ultimo[1] in "{,":
                    esp = "chave"
                else:
                    esp = "identificador"
            yield (esp, w)
            ultimo = ("palavra", w)
            i = m.end()
            continue
        if s.startswith("?.", i) and not (i + 2 < n and s[i + 2].isdigit()):
            yield ("pontuacao", "?.")
            ultimo = ("pont", "?.")
            i += 2
            continue
        yield ("pontuacao", c)
        ultimo = ("pont", c)
        i += 1


def main():
    args = sys.argv[1:]
    top = 30
    if "--top" in args:
        k = args.index("--top")
        top = int(args[k + 1])
        del args[k : k + 2]
    for caminho in args:
        s = open(caminho, encoding="utf-8").read()
        total = len(s.encode("utf-8"))
        por = collections.Counter()
        props = collections.Counter()
        strs = collections.Counter()
        idents = collections.Counter()
        for esp, t in tokens(s):
            b = len(t.encode("utf-8"))
            por[esp] += b
            if esp in ("propriedade", "chave"):
                props[t] += b
            elif esp in ("string", "template"):
                strs[t] += b
            elif esp == "identificador":
                idents[t] += b
        print(f"== {caminho}: {total} bytes")
        for esp, b in por.most_common():
            print(f"  {esp:15} {b:>11,} {100 * b / total:5.1f}%")
        print(f"  {'(espaço)':15} {total - sum(por.values()):>11,}")
        print(f"-- propriedades/chaves mais caras ({len(props)} nomes distintos)")
        for t, b in props.most_common(top):
            print(f"  {b:>9,}  {t}")
        print(f"-- strings mais caras ({len(strs)} distintas)")
        for t, b in strs.most_common(top):
            print(f"  {b:>9,}  {t[:100]}")
        print(f"-- identificadores mais caros ({len(idents)} distintos)")
        for t, b in idents.most_common(top):
            print(f"  {b:>9,}  {t}")


if __name__ == "__main__":
    main()
