#!/usr/bin/env python3
"""Benchmark do servidor HTTP (`bench/http/servidor.dart`): o AOT do DartForge
contra a VM do Dart (`dart run`) e o `dart compile exe`.

Cada executor sobe o servidor (`dart:io` de verdade, keep-alive ligado), que
imprime `porta <n>`; o `wrk` gera a carga nas rotas `/` ("hello" em texto) e
`/json` (`jsonEncode` de um objeto pequeno), com 1 e N conexões. Mede-se:

* req/s e latência p50/p99 (os percentis do `wrk --latency`);
* CPU do servidor por requisição (`utime + stime` de `/proc/<pid>/stat`
  dividido pelas requisições): a medida mais estável quando outros
  processos disputam a máquina;
* RSS em repouso (`VmRSS` depois de subir e responder uma requisição) e RSS
  de pico (`VmHWM` ao fim da carga).

Outros processos disputam a máquina: as repetições alternam os executores e
o relatório dá a mediana e a faixa (mínimo–máximo). Antes de cada medida há
um aquecimento curto (o JIT da VM otimiza as funções quentes nele).

Uso: scripts/bench-http.py [--repeticoes N] [--duracao S] [--conexoes 1,64] [--df-antes EXE]
                            [--df-exe EXE] [--so df,vm,aot] [--saida ARQ.md]

O `wrk` vem do gerenciador de pacotes (`apt-get install wrk`).
"""
import argparse, os, platform, re, shutil, statistics, subprocess, sys, tempfile, time, urllib.request

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTE = os.path.join(RAIZ, "bench", "http", "servidor.dart")
DF = os.environ.get("DARTFORGE", os.path.join(RAIZ, "target", "release", "dartforge"))
ROTAS = [("/", "hello"), ("/json", None)]
NOMES = {"df0": "DartForge AOT (antes)", "df": "DartForge AOT", "vm": "Dart VM (JIT)", "aot": "Dart AOT"}


def rss_kb(pid, campo):
    with open(f"/proc/{pid}/status") as f:
        for l in f:
            if l.startswith(campo + ":"):
                return int(l.split()[1])
    return 0


def subir(cmd):
    p = subprocess.Popen(cmd + ["0"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    linha = p.stdout.readline()
    m = re.match(r"porta (\d+)", linha)
    if not m:
        p.kill()
        raise RuntimeError(f"{cmd}: não subiu: {linha!r} {p.stderr.read()[-2000:]}")
    return p, int(m.group(1))


def pedir(porta, rota):
    with urllib.request.urlopen(f"http://127.0.0.1:{porta}{rota}", timeout=10) as r:
        return r.read().decode()


def cpu_s(pid):
    """utime + stime do processo (todas as threads), em segundos."""
    with open(f"/proc/{pid}/stat") as f:
        campos = f.read().rsplit(")", 1)[1].split()
    return (int(campos[11]) + int(campos[12])) / os.sysconf("SC_CLK_TCK")


def unidade(v):
    m = re.match(r"([\d.]+)(us|ms|s)$", v)
    x = float(m.group(1))
    return x / 1000 if m.group(2) == "us" else x if m.group(2) == "ms" else x * 1000


def wrk(porta, rota, conexoes, duracao):
    threads = max(1, min(conexoes, 2))
    p = subprocess.run(["wrk", f"-t{threads}", f"-c{conexoes}", f"-d{duracao}s", "--latency",
                        f"http://127.0.0.1:{porta}{rota}"], capture_output=True, text=True, timeout=duracao + 60)
    s = p.stdout
    rps = float(re.search(r"Requests/sec:\s+([\d.]+)", s).group(1))
    total = int(re.search(r"(\d+) requests in", s).group(1))
    pct = {k: unidade(v) for k, v in re.findall(r"^\s+(50|99)%\s+(\S+)$", s, re.M)}
    erros = re.search(r"Socket errors: (.*)", s)
    non2xx = re.search(r"Non-2xx or 3xx responses: (\d+)", s)
    avisos = []
    if erros:
        avisos.append("socket: " + erros.group(1))
    if non2xx:
        avisos.append("non-2xx: " + non2xx.group(1))
    return rps, pct.get("50"), pct.get("99"), avisos, total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeticoes", type=int, default=3)
    ap.add_argument("--duracao", type=int, default=5, help="segundos de carga por medida")
    ap.add_argument("--aquecer", type=int, default=2, help="segundos de aquecimento por rota")
    ap.add_argument("--conexoes", default="1,64")
    ap.add_argument("--df-exe", help="executável do DartForge já compilado (pula a compilação)")
    ap.add_argument("--df-antes", help="outro executável do DartForge, medido junto (comparação antes/depois)")
    ap.add_argument("--so", default="df,vm,aot")
    ap.add_argument("--saida")
    a = ap.parse_args()
    conexoes = [int(x) for x in a.conexoes.split(",")]
    tmp = tempfile.mkdtemp(prefix="df-bench-http-")
    execs = {}
    try:
        if a.df_antes:
            execs["df0"] = [a.df_antes]
        for k in a.so.split(","):
            if k == "df":
                exe = a.df_exe or os.path.join(tmp, "servidor.df")
                if not a.df_exe:
                    subprocess.run([DF, "aot", FONTE, exe, "--optimize"], check=True)
                execs[k] = [exe]
            elif k == "vm":
                execs[k] = ["dart", "run", FONTE]
            elif k == "aot":
                exe = os.path.join(tmp, "servidor.dart.exe")
                subprocess.run(["dart", "compile", "exe", FONTE, "-o", exe], check=True, capture_output=True)
                execs[k] = [exe]
        # resultados[k][(rota, c)] = [(rps, p50, p99)], repouso[k] = [kB], pico[k] = [kB]
        res = {k: {} for k in execs}
        repouso = {k: [] for k in execs}
        pico = {k: [] for k in execs}
        avisos = []
        for rep in range(a.repeticoes):
            for k, cmd in execs.items():
                p, porta = subir(cmd)
                try:
                    corpo = pedir(porta, "/")
                    assert corpo == "hello", corpo
                    json = pedir(porta, "/json")
                    assert '"mensagem":"Hello, World!"' in json, json
                    time.sleep(0.5)
                    repouso[k].append(rss_kb(p.pid, "VmRSS"))
                    for rota, _ in ROTAS:
                        if a.aquecer:
                            wrk(porta, rota, max(conexoes), a.aquecer)
                        for c in conexoes:
                            antes = cpu_s(p.pid)
                            rps, p50, p99, av, total = wrk(porta, rota, c, a.duracao)
                            cpu = (cpu_s(p.pid) - antes) / max(total, 1) * 1e6
                            res[k].setdefault((rota, c), []).append((rps, p50, p99, cpu))
                            avisos += [f"{NOMES[k]} {rota} c={c}: {x}" for x in av]
                            print(f"[{rep + 1}] {NOMES[k]:14} {rota:6} c={c:<4} {rps:10.0f} req/s  "
                                  f"p50 {p50:.3f} ms  p99 {p99:.3f} ms  CPU {cpu:.0f} us/req", flush=True)
                    pico[k].append(rss_kb(p.pid, "VmHWM"))
                    print(f"[{rep + 1}] {NOMES[k]:14} RSS repouso {repouso[k][-1] / 1024:.1f} MB, "
                          f"pico {pico[k][-1] / 1024:.1f} MB", flush=True)
                finally:
                    p.kill()
                    p.wait()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    def faixa(vs, fmt):
        return f"{fmt(statistics.median(vs))} [{fmt(min(vs))}–{fmt(max(vs))}]"

    versao = subprocess.run(["dart", "--version"], capture_output=True, text=True)
    ks = list(execs)
    out = [f"Máquina: {platform.platform()}, {os.cpu_count()} CPUs; {(versao.stdout or versao.stderr).strip()}; "
           f"wrk ({a.duracao} s por medida, {a.aquecer} s de aquecimento), {a.repeticoes} repetições alternadas. "
           "Mediana e, entre colchetes, a faixa (mínimo–máximo).", ""]
    for titulo, i, fmt in [("req/s (maior é melhor)", 0, lambda x: f"{x:.0f}"),
                           ("latência p50, ms", 1, lambda x: f"{x:.3f}"),
                           ("latência p99, ms", 2, lambda x: f"{x:.3f}"),
                           ("CPU do servidor por requisição, µs (menor é melhor; menos sensível à disputa da máquina)",
                            3, lambda x: f"{x:.0f}")]:
        out += [f"**{titulo}**", "", "| rota | conexões | " + " | ".join(NOMES[k] for k in ks) + " |",
                "|---|---:|" + "---:|" * len(ks)]
        for rota, _ in ROTAS:
            for c in conexoes:
                cel = [faixa([r[i] for r in res[k][(rota, c)]], fmt) for k in ks]
                out.append(f"| `{rota}` | {c} | " + " | ".join(cel) + " |")
        out.append("")
    out += ["**RSS, MB**", "", "| medida | " + " | ".join(NOMES[k] for k in ks) + " |", "|---|" + "---:|" * len(ks)]
    mb = lambda x: f"{x / 1024:.1f}"
    out.append("| repouso | " + " | ".join(faixa(repouso[k], mb) for k in ks) + " |")
    out.append("| pico | " + " | ".join(faixa(pico[k], mb) for k in ks) + " |")
    if avisos:
        out += ["", "**Avisos do wrk:**", ""] + [f"- {x}" for x in avisos]
    texto = "\n".join(out)
    print("\n" + texto)
    if a.saida:
        open(a.saida, "w").write(texto + "\n")


if __name__ == "__main__":
    sys.exit(main())
