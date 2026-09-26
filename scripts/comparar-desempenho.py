#!/usr/bin/env python3
"""Compara o desempenho de quatro executores nos programas de
`bench/desempenho`: o JIT e o AOT (`--optimize`) do DartForge, a VM do Dart
(`dart run`, JIT) e o AOT oficial (`dart compile exe`).

Cada programa imprime, por núcleo, o tempo de cada rodada e o resultado
(`comum.dart`). O resultado tem de ser igual nos quatro; o tempo estável é a
mediana das rodadas depois da primeira (aquecimento), e a mediana entre as
repetições. As repetições alternam os executores, e o tempo total do processo
(com início e compilação do JIT) sai à parte.

Uso: scripts/comparar-desempenho.py [--repeticoes N] [--saida ARQ.md] [filtro...]
"""
import argparse, os, re, shutil, statistics, subprocess, sys, tempfile, time, platform

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BENCH = os.path.join(RAIZ, "bench", "desempenho")
DF = os.path.join(RAIZ, "target", "release", "dartforge")
LINHA = re.compile(r"^(\w+): ([\d ]+) us \| (.*)$")


def rodar(cmd, prazo=600):
    t0 = time.perf_counter()
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=prazo)
    total = time.perf_counter() - t0
    if p.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)}: saída {p.returncode}\n{p.stderr[-2000:]}")
    return p.stdout, total


def ler(saida):
    r = {}
    for l in saida.splitlines():
        m = LINHA.match(l.strip())
        if m:
            tempos = [int(x) for x in m.group(2).split()]
            r[m.group(1)] = (statistics.median(tempos[1:] or tempos), m.group(3))
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeticoes", type=int, default=3)
    ap.add_argument("--saida")
    ap.add_argument("filtro", nargs="*")
    a = ap.parse_args()
    progs = sorted(f[:-5] for f in os.listdir(BENCH) if f.endswith(".dart") and f != "comum.dart")
    if a.filtro:
        progs = [p for p in progs if any(f in p for f in a.filtro)]
    tmp = tempfile.mkdtemp(prefix="df-bench-")
    nomes = ["DartForge JIT", "DartForge AOT", "Dart VM (JIT)", "Dart AOT"]
    linhas, totais, erros = [], [], []
    for prog in progs:
        fonte = os.path.join(BENCH, prog + ".dart")
        exe_df = os.path.join(tmp, prog + ".df")
        exe_dart = os.path.join(tmp, prog + ".dart.exe")
        rodar([DF, "aot", fonte, exe_df, "--optimize"])
        rodar(["dart", "compile", "exe", fonte, "-o", exe_dart])
        cmds = [[DF, "run", fonte], [exe_df], ["dart", "run", fonte], [exe_dart]]
        por = [[] for _ in cmds]
        tot = [[] for _ in cmds]
        for _ in range(a.repeticoes):
            for i, c in enumerate(cmds):
                s, t = rodar(c)
                por[i].append(ler(s))
                tot[i].append(t)
        totais.append((prog, [statistics.median(t) for t in tot]))
        for nucleo in por[3][0]:
            resultados = {p[0].get(nucleo, (0, "?"))[1] for p in por}
            if len(resultados) != 1:
                erros.append(f"{prog}/{nucleo}: resultados diferentes {resultados}")
            med = [statistics.median(r[nucleo][0] for r in p if nucleo in r) / 1000 for p in por]
            linhas.append((f"{prog}/{nucleo}", med))
            print(f"{prog}/{nucleo}: " + "  ".join(f"{n} {m:.1f}" for n, m in zip(nomes, med)), flush=True)
    shutil.rmtree(tmp, ignore_errors=True)

    versao = subprocess.run(["dart", "--version"], capture_output=True, text=True)
    out = [f"Máquina: {platform.platform()}, {os.cpu_count()} CPUs; {(versao.stdout or versao.stderr).strip()}; "
           f"{a.repeticoes} repetições alternadas.", "",
           "Tempo estável por núcleo (ms, mediana; menor é melhor). Razão = DartForge AOT / Dart AOT.", "",
           "| núcleo | " + " | ".join(nomes) + " | razão |", "|---|" + "---:|" * (len(nomes) + 1)]
    for n, m in linhas:
        razao = m[1] / m[3] if m[3] else float("inf")
        out.append(f"| {n} | " + " | ".join(f"{x:.1f}" for x in m) + f" | {razao:.2f}x |")
    out += ["", "Tempo total do processo (s, mediana; inclui início e, nos JITs, compilação):", "",
            "| programa | " + " | ".join(nomes) + " |", "|---|" + "---:|" * len(nomes)]
    for p, t in totais:
        out.append(f"| {p} | " + " | ".join(f"{x:.2f}" for x in t) + " |")
    if erros:
        out += ["", "**Resultados divergentes:**", ""] + [f"- {e}" for e in erros]
    texto = "\n".join(out)
    print("\n" + texto)
    if a.saida:
        open(a.saida, "w").write(texto + "\n")
    return 1 if erros else 0


if __name__ == "__main__":
    sys.exit(main())
