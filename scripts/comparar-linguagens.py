#!/usr/bin/env python3
"""Compara o DartForge com outros compiladores nativos (RELATORIO-COMPARATIVO-NATIVO.md).

Compila `bench/comparativo/{ola,bench}.<ext>` com cada ferramenta encontrada e
mede: tamanho do executável do `ola`, o tempo de cada núcleo do `bench`
(mediana das rodadas depois da primeira), o RSS de pico do `bench` e 10
partidas do `ola`. Os resultados de cada núcleo têm de ser iguais entre as
ferramentas; uma diferença invalida a linha.

As ferramentas vêm de variáveis de ambiente (ou do PATH, quando o nome cabe):

    DARTFORGE_BIN  dartforge (padrão: target/release/dartforge)
    DART           dart (dart compile exe)
    GO             go          TINYGO   tinygo
    SWIFTC         swiftc      PERRY    perry        SCRIPTC  scriptc

Uso: scripts/comparar-linguagens.py <dir de saída> [ferramenta...]
"""
import os, re, shutil, statistics, subprocess, sys, time

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTES = os.path.join(RAIZ, "bench", "comparativo")
LINHA = re.compile(r"^(\w+): ([\d ]+) ms \| (.*)$")


def ferramenta(var, nome, padrao=None):
    caminho = os.environ.get(var) or (padrao if padrao and os.path.exists(padrao) else None) or shutil.which(nome)
    return caminho


def compiladores():
    df = ferramenta("DARTFORGE_BIN", "dartforge", os.path.join(RAIZ, "target", "release", "dartforge"))
    dart, go, tinygo = ferramenta("DART", "dart"), ferramenta("GO", "go"), ferramenta("TINYGO", "tinygo")
    swiftc, perry, scriptc = ferramenta("SWIFTC", "swiftc"), ferramenta("PERRY", "perry"), ferramenta("SCRIPTC", "scriptc")
    # nome -> (extensão, função(fonte, saída) -> comando)
    c = {}
    if df:
        c["dartforge"] = ("dart", lambda f, o: [df, "aot", f, o, "--optimize"])
    if dart:
        c["dart"] = ("dart", lambda f, o: [dart, "compile", "exe", f, "-o", o])
    if go:
        c["go"] = ("go", lambda f, o: [go, "build", "-trimpath", "-ldflags=-s -w", "-o", o, f])
    if tinygo:
        c["tinygo"] = ("go", lambda f, o: [tinygo, "build", "-opt=2", "-no-debug", "-o", o, f])
    if swiftc:
        c["swift"] = ("swift", lambda f, o: [swiftc, "-O", f, "-o", o])
    if perry:
        c["perry"] = ("ts", lambda f, o: [perry, "compile", f, "-o", o])
    if scriptc:
        c["scriptc"] = ("ts", lambda f, o: [scriptc, "build", f, "-o", o])
    return c


def rodar(exe):
    r = subprocess.run(["/usr/bin/time", "-l", exe], capture_output=True, text=True)
    nucleos = {}
    for l in r.stdout.splitlines():
        m = LINHA.match(l.strip())
        if m:
            t = [int(x) for x in m.group(2).split()]
            nucleos[m.group(1)] = (statistics.median(t[1:] or t), m.group(3))
    rss = re.search(r"(\d+)\s+maximum resident set size", r.stderr)
    return r.returncode, nucleos, int(rss.group(1)) if rss else 0


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    saida = os.path.abspath(sys.argv[1])
    os.makedirs(saida, exist_ok=True)
    todos = compiladores()
    escolhidos = [n for n in todos if len(sys.argv) == 2 or n in sys.argv[2:]]
    linhas, referencia = [], {}
    for nome in escolhidos:
        ext, cmd = todos[nome]
        exes = {}
        for p in ("ola", "bench"):
            exe = os.path.join(saida, f"{p}-{nome}")
            r = subprocess.run(cmd(os.path.join(FONTES, f"{p}.{ext}"), exe), cwd=FONTES, capture_output=True, text=True)
            if r.returncode != 0 or not os.path.exists(exe):
                print(f"{nome}: falha ao compilar {p}: {(r.stderr or r.stdout)[-400:]}", file=sys.stderr)
                break
            exes[p] = exe
        if len(exes) < 2:
            continue
        cod, nucleos, rss = rodar(exes["bench"])
        t0 = time.perf_counter()
        for _ in range(10):
            subprocess.run([exes["ola"]], capture_output=True)
        partida = time.perf_counter() - t0
        diverge = [n for n, (_, r) in nucleos.items() if referencia.setdefault(n, r) != r]
        linhas.append((nome, os.path.getsize(exes["ola"]), nucleos, rss, partida, cod, diverge))
    nomes = ["fib", "arvores", "numerico", "textos"]
    print("| ferramenta | `ola` (bytes) | " + " | ".join(f"{n} (ms)" for n in nomes) + " | RSS do bench (MB) | 10× `ola` (s) |")
    print("|---|---:|" + "---:|" * (len(nomes) + 2))
    for nome, tam, nucleos, rss, partida, cod, diverge in linhas:
        cel = [f"{nucleos[n][0]:g}" if n in nucleos else "—" for n in nomes]
        nota = "" if cod == 0 and not diverge else f" (saída {cod}; difere em {', '.join(diverge)})"
        print(f"| {nome}{nota} | {tam:,} | " + " | ".join(cel) + f" | {rss / 1048576:.0f} | {partida:.2f} |")


if __name__ == "__main__":
    main()
