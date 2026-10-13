#!/usr/bin/env python3
"""Compara o DartForge com outros compiladores nativos (RELATORIO-COMPARATIVO-NATIVO.md).

Compila `bench/comparison/{ola,bench}.<ext>` com cada ferramenta encontrada e
mede: tamanho do executável do `ola`, o tempo de cada núcleo do `bench`
(o menor das rodadas depois da primeira), o RSS de pico do `bench` e 10
partidas do `ola`. Os resultados de cada núcleo têm de ser iguais entre as
ferramentas; uma diferença invalida a linha.

O DartForge entra em várias variantes (`dartforge-<modo>`): A0 é o padrão
(tracing, pilha-sombra, exceções por checagem); A1 com exceções por tabelas;
B0/B1 com raízes por mapas de pilha; ARC (legado), ARCP (preparado) e
ARCP-tab; o sufixo `-dev` é sem `--optimize`; `jit` roda pelo `dartforge run`
(sem executável: a partida inclui a compilação).

As ferramentas vêm de variáveis de ambiente (ou do PATH, quando o nome cabe):

    DARTFORGE_BIN  dartforge (padrão: target/release/dartforge)
    DARTFORGE_JIT  dartforge compilado com `--features jit` (para a variante jit)
    DART           dart (dart compile exe)
    GO             go          TINYGO   tinygo
    SWIFTC         swiftc      PERRY    perry        SCRIPTC  scriptc

Uso: scripts/comparar-linguagens.py <dir de saída> [ferramenta...]
"""
import os, re, shutil, subprocess, sys, time

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTES = os.path.join(RAIZ, "bench", "comparison")
LINHA = re.compile(r"^(\w+): ([\d ]+) ms \| (.*)$")


def ferramenta(var, nome, padrao=None):
    caminho = os.environ.get(var) or (padrao if padrao and os.path.exists(padrao) else None) or shutil.which(nome)
    return caminho


def compiladores():
    df = ferramenta("DARTFORGE_BIN", "dartforge", os.path.join(RAIZ, "target", "release", "dartforge"))
    dart, go, tinygo = ferramenta("DART", "dart"), ferramenta("GO", "go"), ferramenta("TINYGO", "tinygo")
    swiftc, perry, scriptc = ferramenta("SWIFTC", "swiftc"), ferramenta("PERRY", "perry"), ferramenta("SCRIPTC", "scriptc")
    # nome -> (extensão, função(fonte, saída) -> comando[, ambiente])
    c = {}
    if df:
        variantes = [
            ("A0", [], {}), ("A0-dev", None, {}), ("A1", ["--excecoes", "tabelas"], {}),
            ("B0", [], {"DARTFORGE_ROOTS": "mapas"}),
            ("B1", ["--excecoes", "tabelas"], {"DARTFORGE_ROOTS": "mapas"}),
            ("ARC", ["--memoria", "arc"], {}),
            ("ARCP", ["--memoria", "arc"], {"DARTFORGE_ARC_PREPARED": "1"}),
            ("ARCP-tab", ["--memoria", "arc", "--excecoes", "tabelas"], {"DARTFORGE_ARC_PREPARED": "1"}),
        ]
        # As raízes por mapas exigem o gerador embutido (a CLI com `jit`).
        jit = ferramenta("DARTFORGE_JIT", "dartforge-jit")
        for modo, extra, amb in variantes:
            opcoes = ["--optimize"] + (extra or [])
            if extra is None:
                opcoes = []
            cli = jit if "DARTFORGE_ROOTS" in amb and jit else df
            c[f"dartforge-{modo}"] = ("dart", lambda f, o, opcoes=opcoes, cli=cli: [cli, "aot", f, o] + opcoes, amb)
    jit = ferramenta("DARTFORGE_JIT", "dartforge-jit")
    if jit:
        c["dartforge-jit"] = ("dart", None, {"JIT": jit})
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


def rodar(cmd):
    r = subprocess.run(["/usr/bin/time", "-l"] + cmd, capture_output=True, text=True)
    nucleos = {}
    for l in r.stdout.splitlines():
        m = LINHA.match(l.strip())
        if m:
            t = [int(x) for x in m.group(2).split()]
            # O menor tempo depois da primeira rodada: menos sensível a outros
            # processos na máquina que a mediana.
            nucleos[m.group(1)] = (min(t[1:] or t), m.group(3))
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
        ext, cmd, *resto = todos[nome]
        amb = dict(os.environ, **(resto[0] if resto else {}))
        exes = {}
        if cmd is None:
            # JIT: a "execução" é o `dartforge run` da fonte.
            jit = amb.pop("JIT", None)
            exes = {p: [jit, "run", os.path.join(FONTES, f"{p}.{ext}")] for p in ("ola", "bench")} if jit else {}
        else:
            for p in ("ola", "bench"):
                exe = os.path.join(saida, f"{p}-{nome}")
                r = subprocess.run(cmd(os.path.join(FONTES, f"{p}.{ext}"), exe), cwd=FONTES, capture_output=True, text=True, env=amb)
                if r.returncode != 0 or not os.path.exists(exe):
                    print(f"{nome}: falha ao compilar {p}: {(r.stderr or r.stdout)[-400:]}", file=sys.stderr)
                    break
                exes[p] = [exe]
        if len(exes) < 2:
            continue
        cod, nucleos, rss = rodar(exes["bench"])
        t0 = time.perf_counter()
        for _ in range(10):
            subprocess.run(exes["ola"], capture_output=True)
        partida = time.perf_counter() - t0
        diverge = [n for n, (_, r) in nucleos.items() if referencia.setdefault(n, r) != r]
        tamanho = os.path.getsize(exes["ola"][0]) if cmd is not None else None
        linhas.append((nome, tamanho, nucleos, rss, partida, cod, diverge))
    nomes = ["fib", "arvores", "numerico", "textos", "json"]
    print("| ferramenta | `ola` (bytes) | " + " | ".join(f"{n} (ms)" for n in nomes) + " | RSS do bench (MB) | 10× `ola` (s) |")
    print("|---|---:|" + "---:|" * (len(nomes) + 2))
    for nome, tam, nucleos, rss, partida, cod, diverge in linhas:
        cel = [f"{nucleos[n][0]:g}" if n in nucleos else "—" for n in nomes]
        nota = "" if cod == 0 and not diverge else f" (saída {cod}; difere em {', '.join(diverge)})"
        tam = "—" if tam is None else f"{tam:,}"
        print(f"| {nome}{nota} | {tam} | " + " | ".join(cel) + f" | {rss / 1048576:.0f} | {partida:.2f} |")


if __name__ == "__main__":
    main()
