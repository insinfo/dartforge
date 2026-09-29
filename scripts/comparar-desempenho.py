#!/usr/bin/env python3
"""Compara o desempenho de quatro executores nos programas de
`bench/desempenho`: o JIT e o AOT (`--optimize`) do DartForge, a VM do Dart
(`dart run`, JIT) e o AOT oficial (`dart compile exe`).

Cada programa imprime, por núcleo, o tempo de cada rodada e o resultado
(`comum.dart`). O resultado tem de ser igual nos quatro; o tempo estável é a
mediana das rodadas depois da primeira (aquecimento), e a mediana entre as
repetições. As repetições alternam os executores, e o tempo total do processo
(com início e compilação do JIT) e o pico de memória residente (RSS, o
`ru_maxrss` do `wait4`, o mesmo do `/usr/bin/time -v`) saem à parte.

Uso: scripts/comparar-desempenho.py [--repeticoes N] [--saida ARQ.md]
     [--sem-jit] [filtro...]
"""
import argparse, os, re, shutil, signal, statistics, subprocess, sys, tempfile, threading, time, platform

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BENCH = os.path.join(RAIZ, "bench", "desempenho")
DF = os.path.join(RAIZ, "target", "release", "dartforge")
LINHA = re.compile(r"^(\w+): ([\d ]+) us \| (.*)$")


def rodar(cmd, prazo=600, com_rss=False):
    """Executa `cmd`; devolve (saída, segundos) ou, com `com_rss`, também o
    pico de memória residente em MB (o `ru_maxrss` do próprio processo, pelo
    `wait4`)."""
    t0 = time.perf_counter()
    with tempfile.TemporaryFile() as fo, tempfile.TemporaryFile() as fe:
        p = subprocess.Popen(cmd, stdout=fo, stderr=fe)
        esgotou = threading.Event()
        def matar():
            esgotou.set()
            try:
                p.send_signal(signal.SIGKILL)
            except ProcessLookupError:
                pass
        relogio = threading.Timer(prazo, matar)
        relogio.start()
        _, estado, uso = os.wait4(p.pid, 0)
        relogio.cancel()
        p.returncode = os.waitstatus_to_exitcode(estado)
        total = time.perf_counter() - t0
        fo.seek(0)
        fe.seek(0)
        saida = fo.read().decode(errors="replace")
        erro = fe.read().decode(errors="replace")
    rss = uso.ru_maxrss / 1024
    if esgotou.is_set():
        # Registrado como tempo esgotado; a comparação segue.
        return (None, float(prazo), None) if com_rss else (None, float(prazo))
    if p.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)}: saída {p.returncode}\n{erro[-2000:]}")
    return (saida, total, rss) if com_rss else (saida, total)


def ler(saida):
    r = {}
    if saida is None:
        return r
    for l in saida.splitlines():
        m = LINHA.match(l.strip())
        if m:
            tempos = [int(x) for x in m.group(2).split()]
            estaveis = tempos[1:] or tempos
            r[m.group(1)] = (statistics.median(estaveis), m.group(3), min(estaveis), max(estaveis))
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeticoes", type=int, default=3)
    ap.add_argument("--saida")
    ap.add_argument("--prazo", type=int, default=600, help="segundos por execução")
    ap.add_argument("--sem-jit", action="store_true", help="sem o JIT do DartForge (só AOT, VM e Dart AOT)")
    ap.add_argument("filtro", nargs="*")
    a = ap.parse_args()
    progs = sorted(f[:-5] for f in os.listdir(BENCH) if f.endswith(".dart") and f != "comum.dart")
    if a.filtro:
        progs = [p for p in progs if any(f in p for f in a.filtro)]
    tmp = tempfile.mkdtemp(prefix="df-bench-")
    nomes = ["DartForge JIT", "DartForge AOT", "Dart VM (JIT)", "Dart AOT"]
    if a.sem_jit:
        nomes = nomes[1:]
    linhas, totais, picos, erros = [], [], [], []
    for prog in progs:
        fonte = os.path.join(BENCH, prog + ".dart")
        exe_df = os.path.join(tmp, prog + ".df")
        exe_dart = os.path.join(tmp, prog + ".dart.exe")
        rodar([DF, "aot", fonte, exe_df, "--optimize"])
        rodar(["dart", "compile", "exe", fonte, "-o", exe_dart])
        cmds = [[DF, "run", fonte], [exe_df], ["dart", "run", fonte], [exe_dart]]
        if a.sem_jit:
            cmds = cmds[1:]
        por = [[] for _ in cmds]
        tot = [[] for _ in cmds]
        rss = [[] for _ in cmds]
        for _ in range(a.repeticoes):
            for i, c in enumerate(cmds):
                s, t, r = rodar(c, a.prazo, com_rss=True)
                por[i].append(ler(s))
                tot[i].append(t)
                if r is not None:
                    rss[i].append(r)
        totais.append((prog, [statistics.median(t) for t in tot]))
        picos.append((prog, [(statistics.median(r), min(r), max(r)) if r else None for r in rss]))
        for nucleo in por[-1][0]:
            resultados = {r[nucleo][1] for p in por for r in p if nucleo in r}
            if len(resultados) != 1:
                erros.append(f"{prog}/{nucleo}: resultados diferentes {resultados}")
            med, faixa = [], []
            for p in por:
                vals = [r[nucleo] for r in p if nucleo in r]
                if not vals:
                    med.append(None)
                    faixa.append(None)
                    continue
                med.append(statistics.median(v[0] for v in vals) / 1000)
                faixa.append((min(v[2] for v in vals) / 1000, max(v[3] for v in vals) / 1000))
            linhas.append((f"{prog}/{nucleo}", med, faixa))
            print(f"{prog}/{nucleo}: " + "  ".join(f"{n} {'esgotou' if m is None else f'{m:.1f}'}" for n, m in zip(nomes, med)), flush=True)
    shutil.rmtree(tmp, ignore_errors=True)

    versao = subprocess.run(["dart", "--version"], capture_output=True, text=True)
    out = [f"Máquina: {platform.platform()}, {os.cpu_count()} CPUs; {(versao.stdout or versao.stderr).strip()}; "
           f"{a.repeticoes} repetições alternadas.", "",
           "Tempo estável por núcleo (ms): mediana das rodadas depois da primeira, e entre colchetes a faixa "
           "(mínimo–máximo) de todas as repetições; menor é melhor. `esgotou` = passou do prazo. "
           "Razão = DartForge AOT / Dart AOT.", "",
           "| núcleo | " + " | ".join(nomes) + " | razão |", "|---|" + "---:|" * (len(nomes) + 1)]
    def celula(m, f):
        return "esgotou" if m is None else f"{m:.1f} [{f[0]:.1f}–{f[1]:.1f}]"
    for n, m, f in linhas:
        iaot, idart = nomes.index("DartForge AOT"), nomes.index("Dart AOT")
        razao = f"{m[iaot] / m[idart]:.2f}x" if m[iaot] is not None and m[idart] else "—"
        out.append(f"| {n} | " + " | ".join(celula(x, y) for x, y in zip(m, f)) + f" | {razao} |")
    out += ["", "Tempo total do processo (s, mediana; inclui início e, nos JITs, compilação):", "",
            "| programa | " + " | ".join(nomes) + " |", "|---|" + "---:|" * len(nomes)]
    for p, t in totais:
        out.append(f"| {p} | " + " | ".join(f"{x:.2f}" for x in t) + " |")
    out += ["", "Pico de memória residente (MB, mediana e faixa; `ru_maxrss`, o do `/usr/bin/time -v`):", "",
            "| programa | " + " | ".join(nomes) + " |", "|---|" + "---:|" * len(nomes)]
    for p, r in picos:
        out.append(f"| {p} | " + " | ".join("—" if x is None else f"{x[0]:.1f} [{x[1]:.1f}–{x[2]:.1f}]" for x in r) + " |")
    if erros:
        out += ["", "**Resultados divergentes:**", ""] + [f"- {e}" for e in erros]
    texto = "\n".join(out)
    print("\n" + texto)
    if a.saida:
        open(a.saida, "w").write(texto + "\n")
    return 1 if erros else 0


if __name__ == "__main__":
    sys.exit(main())
