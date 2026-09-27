#!/usr/bin/env python3
"""Escreve a docs/MATRIZ.md (V02) a partir dos TSVs do `dartforge-diferencial --matriz`.

Uso: scripts/matriz.py DIR SAIDA

DIR tem um subdiretório por sistema (`linux-x86_64`, `windows-AMD64`…), cada
um com um `<corpus>.tsv` por corpus (`programa  perfil  estado  detalhe`).
Um par (programa, perfil) que nenhum TSV traz é "não medido": a matriz nunca
completa uma célula por dedução.
"""
import collections
import datetime
import os
import re
import sys

PERFIS = ["js-dev", "js-prod", "aot", "aot-gc-stress", "jit"]
ESTADOS = ["suportado", "recusado", "divergente", "não medido"]
SIMBOLO = {"suportado": "✓", "recusado": "R", "divergente": "D", "não medido": "·"}


def ordem(nome):
    """Ordem natural: `9_x` antes de `10_x`."""
    return [int(t) if t.isdigit() else t for t in re.split(r"(\d+)", nome)]


def ler(dir_):
    """{sistema: {corpus: {(programa, perfil): (estado, detalhe)}}}"""
    dados = {}
    for sistema in sorted(os.listdir(dir_)):
        caminho = os.path.join(dir_, sistema)
        if not os.path.isdir(caminho):
            continue
        for arq in sorted(os.listdir(caminho)):
            if not arq.endswith(".tsv"):
                continue
            corpus = arq[:-4]
            celulas = dados.setdefault(sistema, {}).setdefault(corpus, {})
            with open(os.path.join(caminho, arq), encoding="utf-8") as f:
                for linha in f:
                    partes = linha.rstrip("\n").split("\t")
                    if len(partes) != 4:
                        raise SystemExit(f"{arq}: linha inválida: {linha!r}")
                    programa, perfil, estado, detalhe = partes
                    if estado not in ESTADOS:
                        raise SystemExit(f"{arq}: estado desconhecido {estado!r}")
                    if perfil not in PERFIS:
                        PERFIS.append(perfil)
                    celulas[(programa, perfil)] = (estado, detalhe)
    return dados


def apis_do_nativo(raiz):
    """[(título, estado)] da seção "1. APIs ausentes" de docs/NATIVOS-PENDENTES.md:
    o estado é o texto em negrito da linha `* Estado:` de cada item."""
    caminho = os.path.join(raiz, "docs", "NATIVOS-PENDENTES.md")
    itens, dentro = [], False
    with open(caminho, encoding="utf-8") as f:
        for linha in f:
            if linha.startswith("## "):
                dentro = linha.startswith("## 1.")
            elif dentro and linha.startswith("### "):
                itens.append([re.sub(r"^### [0-9.]+ ", "", linha.strip()), "sem estado declarado"])
            elif dentro and itens and linha.startswith("* Estado:"):
                m = re.search(r"\*\*(.+?)\*\*", linha)
                itens[-1][1] = m.group(1) if m else linha[len("* Estado:"):].strip()
    return itens


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    dir_, saida = sys.argv[1], sys.argv[2]
    raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    dados = ler(dir_)
    if not dados:
        raise SystemExit(f"nenhum TSV em {dir_}")
    o = []
    o.append("# Matriz de suporte por programa, perfil e sistema")
    o.append("")
    o.append("Gerada por `scripts/matriz.sh` (que roda o `dartforge-diferencial --matriz`")
    o.append("em cada corpus e perfil) e `scripts/matriz.py`; não edite à mão. Data da")
    o.append(f"geração: {datetime.date.today().isoformat()}.")
    o.append("")
    o.append("Cada par (programa, perfil) tem **um** estado, contra a referência do")
    o.append("programa (a VM do SDK que o programa pede; o `dartdevc` quando o programa")
    o.append("só existe na web):")
    o.append("")
    o.append("* **suportado** (✓) — compilou e o stdout e o código de saída são os da referência;")
    o.append("* **recusado** (R) — o perfil recusou na compilação, com diagnóstico: igual à")
    o.append("  referência (\"como a referência\", ex.: biblioteca que não existe na")
    o.append("  plataforma) ou não (o recurso falta no perfil);")
    o.append("* **divergente** (D) — executou e a saída difere (inclusive os listados em")
    o.append("  `PENDENTES`);")
    o.append("* **não medido** (·) — o perfil não rodou o programa neste sistema, ou a")
    o.append("  referência não pôde ser obtida. Nada é afirmado sobre ele.")
    o.append("")
    o.append("Os percentuais de um corpus medem **aquele corpus**, não a linguagem: um")
    o.append("programa exercita um conjunto de operações, e operação que nenhum programa")
    o.append("exercita não aparece aqui. As APIs públicas do nativo levantadas fora do")
    o.append("corpus estão na última seção.")
    o.append("")

    o.append("## Resumo")
    o.append("")
    o.append("| sistema | corpus | perfil | programas | suportado | recusado (como a ref.) | recusado (falta) | divergente | não medido |")
    o.append("|---|---|---|---:|---:|---:|---:|---:|---:|")
    problemas = []
    for sistema, corpora in dados.items():
        for corpus, celulas in sorted(corpora.items()):
            programas = sorted({p for (p, _) in celulas})
            for perfil in PERFIS:
                cont = collections.Counter()
                medidos = 0
                for p in programas:
                    e = celulas.get((p, perfil))
                    if e is None:
                        cont["não medido"] += 1
                        continue
                    medidos += 1
                    estado, detalhe = e
                    if estado == "recusado":
                        cont["recusado-ref" if detalhe == "como a referência" else "recusado-falta"] += 1
                    else:
                        cont[estado] += 1
                    if estado == "divergente" or (estado == "recusado" and detalhe != "como a referência"):
                        problemas.append((sistema, corpus, perfil, p, estado, detalhe))
                if medidos == 0:
                    continue
                o.append(
                    f"| {sistema} | {corpus} | {perfil} | {len(programas)} | {cont['suportado']} | "
                    f"{cont['recusado-ref']} | {cont['recusado-falta']} | {cont['divergente']} | {cont['não medido']} |"
                )
    perfis_medidos = {perfil for corpora in dados.values() for cel in corpora.values() for (_, perfil) in cel}
    faltam = [p for p in PERFIS if p not in perfis_medidos]
    o.append("")
    o.append("Sistemas sem TSV aqui não foram medidos por esta matriz (o CI deles tem o")
    o.append("placar próprio, que não separa os estados).")
    if faltam:
        o.append(f"Perfis sem nenhuma medida: {', '.join(faltam)}.")
    o.append("")

    o.append("## Recusados por falta e divergentes")
    o.append("")
    if problemas:
        o.append("| sistema | corpus | perfil | programa | estado | detalhe |")
        o.append("|---|---|---|---|---|---|")
        for s, c, perfil, p, e, d in problemas:
            d = d.replace("|", "\\|")
            o.append(f"| {s} | {c} | {perfil} | `{p}` | {e} | {d} |")
    else:
        o.append("Nenhum: todo programa medido é suportado ou recusado como a referência.")
    o.append("")

    o.append("## Por programa")
    o.append("")
    o.append("✓ suportado · R recusado · D divergente · `·` não medido.")
    for sistema, corpora in dados.items():
        for corpus, celulas in sorted(corpora.items()):
            perfis = [p for p in PERFIS if any(pp == p for (_, pp) in celulas)]
            o.append("")
            o.append(f"### {sistema} — corpus/{corpus}")
            o.append("")
            o.append("| programa | " + " | ".join(perfis) + " |")
            o.append("|---|" + "|".join(":---:" for _ in perfis) + "|")
            for p in sorted({p for (p, _) in celulas}, key=ordem):
                linha = []
                for perfil in perfis:
                    e = celulas.get((p, perfil))
                    linha.append(SIMBOLO[e[0]] if e else SIMBOLO["não medido"])
                o.append(f"| `{p}` | " + " | ".join(linha) + " |")
    o.append("")

    o.append("## APIs públicas do levantamento de natives (backend nativo)")
    o.append("")
    o.append("As operações que ficaram fora do nativo até a auditoria de 2026-09-27, com o")
    o.append("estado declarado em `docs/NATIVOS-PENDENTES.md` §1 (o detalhe e a evidência")
    o.append("de cada uma estão lá; \"ausente\" é `UnsupportedError(\"não suportado no")
    o.append("backend nativo: …\")` em execução, no AOT e no JIT):")
    o.append("")
    o.append("| API | estado no nativo |")
    o.append("|---|---|")
    for titulo, estado in apis_do_nativo(raiz):
        o.append(f"| {titulo} | {estado} |")
    o.append("")
    with open(saida, "w", encoding="utf-8") as f:
        f.write("\n".join(o))
    print(f"{saida}: {sum(len(c) for cs in dados.values() for c in cs.values())} células de {len(dados)} sistema(s)")


if __name__ == "__main__":
    main()
