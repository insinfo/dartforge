#!/usr/bin/env python3
"""Servidor HTTP do comparativo (RELATORIO-COMPARATIVO-NATIVO.md): o mesmo
protocolo de `bench/http/servidor.dart` em Dart (DartForge e Dart), Go, Swift
(SwiftNIO) e TypeScript (Perry, scriptc), sob a carga de
`bench/comparison/carga.go` (keep-alive; substitui o `wrk` no macOS).

Para cada servidor: sobe na porta 0, aquece, e mede `/` e `/json` com 1 e N
conexões: req/s, latência p50/p99, CPU do servidor por mil requisições
(`ps -o cputime`, a medida menos sensível a outros processos) e RSS depois
da carga. O gerador de carga roda na mesma máquina e disputa os núcleos.

Ferramentas como em `scripts/comparar-linguagens.py` (DARTFORGE_BIN,
DARTFORGE_JIT, DART, GO, SWIFT, PERRY, SCRIPTC, TINYGO).

Uso: scripts/comparar-http.py <dir de saída> [--duracao S] [--conexoes 1,32] [servidor...]
"""
import argparse, os, re, shutil, subprocess, sys, time, urllib.request

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTES = os.path.join(RAIZ, "bench", "comparison")


def ferramenta(var, nome, padrao=None):
    return os.environ.get(var) or (padrao if padrao and os.path.exists(padrao) else None) or shutil.which(nome)


def servidores(saida):
    """nome -> (comando de build ou None, comando de execução, ambiente)."""
    df = ferramenta("DARTFORGE_BIN", "dartforge", os.path.join(RAIZ, "target", "release", "dartforge"))
    jit = ferramenta("DARTFORGE_JIT", "dartforge-jit")
    dart, go, swift = ferramenta("DART", "dart"), ferramenta("GO", "go"), ferramenta("SWIFT", "swift")
    perry, scriptc, tinygo = ferramenta("PERRY", "perry"), ferramenta("SCRIPTC", "scriptc"), ferramenta("TINYGO", "tinygo")
    fonte = lambda ext: os.path.join(FONTES, f"servidor.{ext}")
    exe = lambda nome: os.path.join(saida, f"srv-{nome}")
    s = {}
    for modo, cli, extra, amb in [
        ("A0", df, [], {}), ("A1", df, ["--excecoes", "tabelas"], {}),
        ("B1", jit, ["--excecoes", "tabelas"], {"DARTFORGE_ROOTS": "mapas"}),
        ("ARC", df, ["--memoria", "arc"], {}),
        ("ARCP", df, ["--memoria", "arc"], {"DARTFORGE_ARC_PREPARED": "1"}),
    ]:
        if cli:
            nome = f"dartforge-{modo}"
            s[nome] = ([cli, "aot", fonte("dart"), exe(nome), "--optimize"] + extra, [exe(nome)], amb)
    if jit:
        s["dartforge-jit"] = (None, [jit, "run", fonte("dart")], {})
    if dart:
        s["dart-aot"] = ([dart, "compile", "exe", fonte("dart"), "-o", exe("dart-aot")], [exe("dart-aot")], {})
        s["dart-vm"] = (None, [dart, "run", fonte("dart")], {})
    if go:
        s["go"] = ([go, "build", "-trimpath", "-ldflags=-s -w", "-o", exe("go"), fonte("go")], [exe("go")], {})
    if tinygo:
        s["tinygo"] = ([tinygo, "build", "-opt=2", "-no-debug", "-o", exe("tinygo"), fonte("go")], [exe("tinygo")], {})
    if swift:
        pacote = os.path.join(FONTES, "swift-servidor")
        s["swift-nio"] = ([swift, "build", "-c", "release", "--package-path", pacote],
                          [os.path.join(pacote, ".build", "release", "servidor")], {})
    if perry:
        s["perry"] = ([perry, "compile", fonte("ts"), "-o", exe("perry")], [exe("perry")], {})
    if scriptc:
        s["scriptc"] = ([scriptc, "build", fonte("ts"), "-o", exe("scriptc")], [exe("scriptc")], {})
    return s


def ps(pid, campo):
    r = subprocess.run(["ps", "-o", f"{campo}=", "-p", str(pid)], capture_output=True, text=True)
    return r.stdout.strip()


def cpu_s(pid):
    # `ps -o cputime`: [[dd-]hh:]mm:ss.cc, somando os processos filhos não.
    v = ps(pid, "cputime")
    if not v:
        return 0.0
    partes = [float(x) for x in re.split("[:-]", v)]
    seg = 0.0
    for p in partes:
        seg = seg * 60 + p
    return seg


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("saida")
    ap.add_argument("--duracao", type=float, default=5)
    ap.add_argument("--conexoes", default="1,32")
    ap.add_argument("filtro", nargs="*")
    a = ap.parse_args()
    saida = os.path.abspath(a.saida)
    os.makedirs(saida, exist_ok=True)
    go = ferramenta("GO", "go")
    carga = os.path.join(saida, "carga")
    subprocess.run([go, "build", "-o", carga, os.path.join(FONTES, "carga.go")], check=True)
    conexoes = [int(c) for c in a.conexoes.split(",")]
    todos = servidores(saida)
    linhas = []
    for nome, (build, cmd, amb) in todos.items():
        if a.filtro and nome not in a.filtro:
            continue
        ambiente = dict(os.environ, **amb)
        if nome == "swift-nio":
            # O SDKROOT do LLVM (o do DartForge) não serve ao toolchain Swift.
            ambiente.pop("SDKROOT", None)
        if build:
            r = subprocess.run(build, capture_output=True, text=True, env=ambiente, cwd=FONTES)
            if r.returncode != 0:
                print(f"{nome}: falha ao compilar: {(r.stderr or r.stdout)[-300:]}", file=sys.stderr)
                continue
        tamanho = os.path.getsize(cmd[0]) if build else None
        proc = subprocess.Popen(cmd + ["0"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=ambiente, cwd=FONTES)
        porta, inicio = None, time.time()
        while time.time() - inicio < 120:
            l = proc.stdout.readline()
            if not l:
                break
            m = re.match(r"porta (\d+)", l.strip())
            if m:
                porta = int(m.group(1))
                break
        if porta is None:
            print(f"{nome}: o servidor não subiu", file=sys.stderr)
            proc.kill()
            continue
        base = f"http://127.0.0.1:{porta}"
        subprocess.run([carga, base + "/json", "8", "1"], capture_output=True)  # aquecimento
        rss_repouso = int(ps(proc.pid, "rss") or 0) // 1024
        for rota in ("/", "/json"):
            for c in conexoes:
                c0 = cpu_s(proc.pid)
                r = subprocess.run([carga, base + rota, str(c), str(a.duracao)], capture_output=True, text=True)
                c1 = cpu_s(proc.pid)
                m = re.search(r"req/s (\d+) p50_us (\d+) p99_us (\d+) erros (\d+)", r.stdout)
                if not m:
                    print(f"{nome} {rota} {c}: {r.stdout} {r.stderr}", file=sys.stderr)
                    continue
                rps, p50, p99, erros = map(int, m.groups())
                cpu_mil = (c1 - c0) * 1000 / max(rps * a.duracao / 1000, 1e-9)
                linhas.append((nome, rota, c, rps, p50, p99, erros, cpu_mil, tamanho, rss_repouso, int(ps(proc.pid, "rss") or 0) // 1024))
        try:
            urllib.request.urlopen(base + "/sair", timeout=2).read()
        except Exception:
            pass
        time.sleep(0.3)
        proc.kill()
    print("| servidor | rota | conexões | req/s | p50 (µs) | p99 (µs) | erros | CPU ms / mil req | executável | RSS repouso → carga (MB) |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---|")
    for nome, rota, c, rps, p50, p99, erros, cpu_mil, tam, r0, r1 in linhas:
        tam = "—" if tam is None else f"{tam / 1048576:.1f} MB"
        print(f"| {nome} | `{rota}` | {c} | {rps:,} | {p50} | {p99} | {erros} | {cpu_mil:.0f} | {tam} | {r0} → {r1} |")


if __name__ == "__main__":
    main()
