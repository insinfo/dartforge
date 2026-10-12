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
out = root / "target/comparacao-arc-sobrevivencia-densa-2a9baa0e"
out.mkdir(exist_ok=False)
old = root / "target/comparacao-arc-trial-denso-687a7fd4/objetos_escapam-ARC.exe"
new = out / "objetos_escapam-ARC.exe"
cli = root / "target/release/dartforge.exe"
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(old) == "82acd4d5a83f5b2baa102eef6e0ead7608bb19d382435aa7c06cb00c89ac159a"
producer = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
assert producer == "2a9baa0ecd834924e7e6b9621cc004c9faa1f1c2"
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
    assert hashlib.sha256(subprocess.check_output(["git", "show", "687a7fd4:bench/desempenho/" + name], cwd=root)).hexdigest() == inputs[name]
command = [str(cli), "aot", str(root / "bench/desempenho/objetos_escapam.dart"), str(new),
           "--optimize", "--excecoes", "checagem", "--memoria=arc"]
manifest = {"fonte_antes": "687a7fd4e062188847c13069eed33f6598751b24", "fonte_depois": producer,
            "escopo": "comparação local de dois kernels entre revisões; não substitui gate ARC/A0 ou isola o runtime",
            "arquivos_crates_alterados": subprocess.check_output(["git", "diff", "--name-only", "687a7fd4", producer, "--", "crates"], cwd=root, text=True).splitlines(), "cli_sha256": sha(cli), "entradas": inputs, "afinidade": "0x4", "repeticoes": 7,
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

