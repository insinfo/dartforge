"""Sete pares ARC antes/depois, sem rastro, com hashes e resultados brutos."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

root = next(p for p in Path(__file__).resolve().parents if (p / "Cargo.toml").is_file())
out = root / "target/comparacao-arc-mortos-b706e0b0"
out.mkdir(exist_ok=False)
old = root / "target/modos-2026-10-10-55beeb34-a0-arc/objetos_escapam-ARC.exe"
new = out / "objetos_escapam-ARC.exe"
cli = root / "target/release/dartforge.exe"
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(old) == "56a2136351e329c4880bc737e61c47f4a44c178d9ef7676998b797c2cf5a8a32"
producer = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
assert producer == "b706e0b0b073dc61af16aa866ce25bc53397dddd"
assert subprocess.check_output(["git", "status", "--porcelain", "--", "crates", "bench/desempenho"], cwd=root, text=True).strip() == ""
env = os.environ.copy()
for name in ["DARTFORGE_GC_STRESS", "DARTFORGE_ARC_CICLOS"]:
    env.pop(name, None)
env.update({"DARTFORGE_RAIZES": "sombra", "DARTFORGE_GC_RASTRO": "0", "DARTFORGE_GC_OFF": "0",
            "DARTFORGE_ARC_CONFERIR": "0", "DARTFORGE_ARC_BERCARIO": "0", "DARTFORGE_HEAP_MAX_MB": "256"})
k = ctypes.windll.kernel32
k.GetCurrentProcess.restype = ctypes.c_void_p
assert k.SetProcessAffinityMask(ctypes.c_void_p(k.GetCurrentProcess()), ctypes.c_size_t(4))
inputs = {}
for name in ["objetos_escapam.dart", "comum.dart"]:
    path = root / "bench/desempenho" / name
    inputs[name] = sha(path)
    assert hashlib.sha256(subprocess.check_output(["git", "show", "55beeb34:bench/desempenho/" + name], cwd=root)).hexdigest() == inputs[name]
command = [str(cli), "aot", str(root / "bench/desempenho/objetos_escapam.dart"), str(new),
           "--optimize", "--excecoes", "checagem", "--memoria=arc"]
manifest = {"fonte_antes": "55beeb34b8c57e44cd7563274b396b7550a5d19d", "fonte_depois": producer,
            "cli_sha256": sha(cli), "entradas": inputs, "afinidade": "0x4", "repeticoes": 7,
            "comando_compilacao": command, "ambiente": {n: env.get(n) for n in
            ["DARTFORGE_RAIZES", "DARTFORGE_GC_RASTRO", "DARTFORGE_GC_OFF", "DARTFORGE_ARC_CONFERIR",
             "DARTFORGE_ARC_BERCARIO", "DARTFORGE_HEAP_MAX_MB", "DARTFORGE_GC_STRESS", "DARTFORGE_ARC_CICLOS"]}}
(out / "manifesto.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
with (out / "build.log").open("wb") as log:
    code = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
assert code == 0 and new.is_file(), "compilação falhou"
print("Executável novo compilado; iniciando sete pares.", flush=True)
bins = {"antes": old, "depois": new}
hashes = {name: sha(path) for name, path in bins.items()}
manifest["executaveis_sha256"] = hashes
(out / "manifesto.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
samples = {mode: {name: [] for name in ["arvores", "lista_ligada"]} for mode in bins}
pattern = re.compile(r"^(\w+): ([\d ]+) us \| (.*)$", re.MULTILINE)
for rep in range(1, 8):
    for mode in (["antes", "depois"] if rep % 2 else ["depois", "antes"]):
        start = time.perf_counter()
        p = subprocess.run([str(bins[mode])], cwd=root / "bench/desempenho", env=env,
                           capture_output=True, text=True, encoding="utf-8", timeout=1800)
        row = {"repeticao": rep, "modo": mode, "codigo": p.returncode, "stdout": p.stdout,
               "stderr": p.stderr, "segundos_processo": time.perf_counter() - start, "nucleos": {}}
        for name, times, result in pattern.findall(p.stdout):
            assert name not in row["nucleos"]
            times = list(map(int, times.split()))
            assert len(times) > 1
            row["nucleos"][name] = {"rodadas_us": times, "mediana_us": statistics.median(times[1:]), "resultado": result}
        with (out / "amostras.jsonl").open("a", encoding="utf-8", newline="\n") as log:
            log.write(json.dumps(row, ensure_ascii=False) + "\n")
        assert p.returncode == 0 and {n: v["resultado"] for n, v in row["nucleos"].items()} == {
            "arvores": "3156655", "lista_ligada": "499999500000"}
        for name, value in row["nucleos"].items():
            samples[mode][name].append(value["mediana_us"])
    print(f"Par {rep}/7 concluído.", flush=True)
assert hashes == {name: sha(path) for name, path in bins.items()}
assert sha(cli) == manifest["cli_sha256"]
results = {}
for name in samples["antes"]:
    a, d = (statistics.median(samples[m][name]) for m in ["antes", "depois"])
    results[name] = {"antes_us": a, "depois_us": d, "depois_antes": d / a,
                     "razoes_pareadas": [d / a for a, d in zip(samples["antes"][name], samples["depois"][name])]}
(out / "resultado.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8", newline="\n")
print(results)
