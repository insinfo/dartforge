#!/usr/bin/env python3
"""Tempo de execução do `bench/desempenho` nas quatro combinações de raízes e
exceções do backend nativo (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §6, §8):

    A0  pilha-sombra + checagem   (o padrão)
    A1  pilha-sombra + tabelas    (`--excecoes tabelas`)
    B0  mapas        + checagem   (`DARTFORGE_RAIZES=mapas`)
    B1  mapas        + tabelas

e o `dart compile exe` como referência. Todos em produção (`aot --optimize`).

Fase 1 compila tudo (pula o executável que já existe na saída); fase 2 roda
`--repeticoes` vezes cada executável, alternando os modos a cada repetição.
Cada programa imprime, por núcleo, o tempo de cada rodada e o resultado
(`comum.dart`); o resultado tem de ser igual em todos. O tempo de um núcleo
numa execução é a mediana das rodadas depois da primeira; o do modo, a mediana
entre as repetições. `amostras.jsonl` preserva stdout, stderr, código de saída
e núcleos de cada execução. Resultado diferente em qualquer repetição,
execução incompleta ou falha de compilação invalida a rodada (código 1).

`--afinidade 0x4` prende a medida (e os filhos, que herdam) aos processadores
da máscara: numa CPU híbrida (núcleos P e E) o mesmo executável varia 20–40%
conforme o núcleo em que o sistema o põe. Só no Windows.

Uso: scripts/medir-modos-desempenho.py <dir de saída> [--repeticoes N]
     [--sem-dart] [--afinidade MASCARA] [--modos A0,ARC] [filtro...]
"""
import argparse, json, os, re, statistics, subprocess, sys, time

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BENCH = os.path.join(RAIZ, "bench", "desempenho")
DF = os.environ.get("DARTFORGE_BIN") or os.path.join(RAIZ, "target", "release", "dartforge.exe")
DART = os.path.join(os.environ.get("DARTFORGE_DART_SDK", "C:/tools/dartsdk-3.6.2"), "bin", "dart.exe")
LINHA = re.compile(r"^(\w+): ([\d ]+) us \| (.*)$")
MODOS = [
    ("A0", "sombra", "checagem", "tracing"),
    ("A1", "sombra", "tabelas", "tracing"),
    ("B0", "mapas", "checagem", "tracing"),
    ("B1", "mapas", "tabelas", "tracing"),
    ("ARC", "sombra", "checagem", "arc"),
]


def log(msg):
    print(msg, flush=True)


def compilar(saida, nome, fonte, modo, raizes, excecoes, memoria):
    exe = os.path.join(saida, f"{nome}-{modo}.exe")
    if os.path.exists(exe):
        return exe, 0.0
    env = dict(os.environ, DARTFORGE_RAIZES=raizes)
    cmd = [DF, "aot", fonte, exe, "--optimize", "--excecoes", excecoes, "--memoria", memoria]
    t0 = time.perf_counter()
    r = subprocess.run(cmd, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
    dt = time.perf_counter() - t0
    if r.returncode != 0 or not os.path.exists(exe):
        log(f"  FALHA ao compilar {nome} {modo}: {r.stderr[-800:]}")
        return None, dt
    return exe, dt


def compilar_dart(saida, nome, fonte):
    exe = os.path.join(saida, f"{nome}-dart.exe")
    if os.path.exists(exe):
        return exe, 0.0
    t0 = time.perf_counter()
    r = subprocess.run([DART, "compile", "exe", fonte, "-o", exe], capture_output=True, text=True, encoding="utf-8", errors="replace")
    dt = time.perf_counter() - t0
    if r.returncode != 0:
        log(f"  FALHA no dart compile exe {nome}: {r.stderr[-800:]}")
        return None, dt
    return exe, dt


def rodar(exe):
    r = subprocess.run([exe], cwd=BENCH, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=1800)
    nucleos = {}
    for l in r.stdout.splitlines():
        m = LINHA.match(l.strip())
        if m:
            tempos = [int(x) for x in m.group(2).split()]
            estaveis = tempos[1:] if len(tempos) > 1 else tempos
            nucleos[m.group(1)] = (statistics.median(estaveis), m.group(3))
    return r.returncode, nucleos, r.stdout, r.stderr


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("saida")
    ap.add_argument("--repeticoes", type=int, default=7)
    ap.add_argument("--sem-dart", action="store_true")
    ap.add_argument("--afinidade", default=None)
    ap.add_argument("--modos", default=None, help="só estes modos, separados por vírgula (A0 entra sempre)")
    ap.add_argument("filtro", nargs="*")
    a = ap.parse_args()
    if a.repeticoes < 1:
        ap.error("--repeticoes deve ser positivo")
    os.makedirs(a.saida, exist_ok=True)
    if a.afinidade:
        import ctypes
        k = ctypes.windll.kernel32
        k.GetCurrentProcess.restype = ctypes.c_void_p
        if not k.SetProcessAffinityMask(ctypes.c_void_p(k.GetCurrentProcess()), ctypes.c_size_t(int(a.afinidade, 0))):
            sys.exit(f"SetProcessAffinityMask({a.afinidade}) falhou")
    programas = sorted(f[:-5] for f in os.listdir(BENCH) if f.endswith(".dart") and f != "comum.dart")
    if a.filtro:
        programas = [p for p in programas if any(x in p for x in a.filtro)]
    if not programas:
        ap.error("nenhum programa corresponde ao filtro")
    escolhidos = MODOS if not a.modos else [m for m in MODOS if m[0] == "A0" or m[0] in a.modos.split(",")]
    modos = [m[0] for m in escolhidos] + ([] if a.sem_dart else ["dart"])

    log("# Fase 1: compilação")
    exes = {}
    for nome in programas:
        fonte = os.path.join(BENCH, nome + ".dart")
        for modo, raizes, excecoes, memoria in escolhidos:
            exe, dt = compilar(a.saida, nome, fonte, modo, raizes, excecoes, memoria)
            log(f"  {nome} {modo}: {'ok' if exe else 'falhou'} ({dt:.0f} s)")
            if exe:
                exes[(nome, modo)] = exe
        if not a.sem_dart:
            exe, dt = compilar_dart(a.saida, nome, fonte)
            log(f"  {nome} dart: {'ok' if exe else 'falhou'} ({dt:.0f} s)")
            if exe:
                exes[(nome, "dart")] = exe

    log("# Fase 2: execução")
    medidas = {}  # (nome, modo, nucleo) -> [mediana por repetição]
    resultados = {}  # (nome, nucleo) -> primeiro resultado observado
    falhas = []
    observados = {}  # (nome, modo, repetição) -> núcleos presentes
    # Um registro por execução, incluindo stdout com todas as rodadas (também
    # o aquecimento). Gravar imediatamente preserva a evidência de uma falha.
    with open(os.path.join(a.saida, "amostras.jsonl"), "w", encoding="utf-8") as arquivo:
        arquivo.write(json.dumps({"tipo": "configuracao", "programas": programas,
                                  "modos": modos, "repeticoes": a.repeticoes,
                                  "afinidade": a.afinidade}, ensure_ascii=False) + "\n")
    for rep in range(a.repeticoes):
        for nome in programas:
            ordem = modos[rep % len(modos):] + modos[: rep % len(modos)]
            for modo in ordem:
                exe = exes.get((nome, modo))
                if not exe:
                    if rep == 0:
                        falhas.append(f"{nome}/{modo}: compilação falhou")
                    continue
                try:
                    cod, nucleos, stdout, stderr = rodar(exe)
                except subprocess.TimeoutExpired as erro:
                    # TimeoutExpired pode entregar bytes mesmo com text=True.
                    def texto_parcial(valor):
                        return valor.decode("utf-8", errors="replace") if isinstance(valor, bytes) else (valor or "")
                    cod, nucleos = "timeout", {}
                    stdout, stderr = texto_parcial(erro.stdout), texto_parcial(erro.stderr)
                except OSError as erro:
                    cod, nucleos, stdout, stderr = "erro de execução", {}, "", str(erro)
                registro = {"tipo": "execucao", "programa": nome, "modo": modo,
                            "repeticao": rep + 1, "executavel": os.path.abspath(exe),
                            "codigo": cod, "nucleos": nucleos,
                            "stdout": stdout, "stderr": stderr}
                with open(os.path.join(a.saida, "amostras.jsonl"), "a", encoding="utf-8") as arquivo:
                    arquivo.write(json.dumps(registro, ensure_ascii=False) + "\n")
                observados[(nome, modo, rep)] = set(nucleos)
                if cod != 0:
                    log(f"  {nome} {modo}: saiu com {cod}")
                    falhas.append(f"{nome}/{modo}, repetição {rep + 1}: saiu com {cod}")
                if not nucleos:
                    falhas.append(f"{nome}/{modo}, repetição {rep + 1}: sem núcleos")
                for n, (t, res) in nucleos.items():
                    medidas.setdefault((nome, modo, n), []).append(t)
                    anterior = resultados.setdefault((nome, n), res)
                    if res != anterior:
                        falhas.append(f"{nome}/{n}/{modo}, repetição {rep + 1}: {res!r} != {anterior!r}")
        log(f"  repetição {rep + 1}/{a.repeticoes}")

    for nome in programas:
        esperados = {n for (p, _, n) in medidas if p == nome}
        for modo in modos:
            for rep in range(a.repeticoes):
                presentes = observados.get((nome, modo, rep), set())
                if presentes != esperados:
                    falhas.append(f"{nome}/{modo}, repetição {rep + 1}: faltam {sorted(esperados - presentes)}")

    linhas = ["| programa/núcleo | " + " | ".join(modos) + " | A1/A0 | B0/A0 | B1/A0 | ARC/A0 | A0/dart |",
              "|---|" + "---:|" * (len(modos) + 5)]
    for nome in programas:
        nucleos = sorted({n for (p, m, n) in medidas if p == nome})
        for n in nucleos:
            med = {m: statistics.median(medidas[(nome, m, n)]) for m in modos if (nome, m, n) in medidas}
            def razao(x, y):
                return f"{med[x] / med[y]:.2f}" if x in med and y in med and med[y] else "—"
            celulas = [f"{med[m] / 1000:.1f}" if m in med else "—" for m in modos]
            linhas.append(f"| {nome}/{n} | " + " | ".join(celulas) + f" | {razao('A1', 'A0')} | {razao('B0', 'A0')} | {razao('B1', 'A0')} | {razao('ARC', 'A0')} | {razao('A0', 'dart')} |")
    # Média geométrica das razões.
    def geo(x, y):
        rs = []
        for (p, m, n) in medidas:
            if m == x and (p, y, n) in medidas:
                a_ = statistics.median(medidas[(p, x, n)])
                b_ = statistics.median(medidas[(p, y, n)])
                if a_ > 0 and b_ > 0:
                    rs.append(a_ / b_)
        return statistics.geometric_mean(rs) if rs else float("nan")
    resumo = [
        "",
        f"Média geométrica: A1/A0 {geo('A1', 'A0'):.3f}, B0/A0 {geo('B0', 'A0'):.3f}, B1/A0 {geo('B1', 'A0'):.3f}, ARC/A0 {geo('ARC', 'A0'):.3f}"
        + ("" if a.sem_dart else f", A0/dart {geo('A0', 'dart'):.3f}"),
        f"Tempos em ms (mediana de {a.repeticoes} execuções alternadas; em cada uma, a mediana das rodadas sem a primeira)"
        + (f"; presos aos processadores {a.afinidade}." if a.afinidade else "."),
    ]
    if falhas:
        resumo.append("Rodada inválida (não usar as razões como comparação): " + "; ".join(falhas))
    else:
        resumo.append("Resultados iguais em todos os modos e repetições; amostras brutas em amostras.jsonl.")
    texto = "\n".join(linhas + resumo)
    with open(os.path.join(a.saida, "resultado.md"), "w", encoding="utf-8") as f:
        f.write(texto + "\n")
    log(texto)
    return 1 if falhas else 0


if __name__ == "__main__":
    sys.exit(main())
