"""Reconstitui a comparação do trial deletion a partir dos resultados brutos."""
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess

root = next(p for p in Path(__file__).resolve().parents if (p / "Cargo.toml").is_file())
out = root / "target/comparacao-arc-trial-denso-687a7fd4"
manifest = json.loads((out / "manifesto.json").read_text(encoding="utf-8"))
rows = [json.loads(line) for line in (out / "amostras.jsonl").read_text(encoding="utf-8").splitlines()]
reported = json.loads((out / "resultado.json").read_text(encoding="utf-8"))
assert manifest["fonte_antes"] == "b706e0b0b073dc61af16aa866ce25bc53397dddd"
assert manifest["fonte_depois"] == "687a7fd4e062188847c13069eed33f6598751b24"
assert manifest["afinidade"] == "0x4" and manifest["repeticoes"] == 7
assert manifest["comando_compilacao"][-4:] == ["--optimize", "--excecoes", "checagem", "--memoria=arc"]
assert manifest["ambiente"] == {
    "DARTFORGE_RAIZES": "sombra", "DARTFORGE_GC_RASTRO": "0",
    "DARTFORGE_GC_OFF": "0", "DARTFORGE_ARC_CONFERIR": "0",
    "DARTFORGE_ARC_BERCARIO": "0", "DARTFORGE_HEAP_MAX_MB": "256",
    "DARTFORGE_GC_STRESS": None, "DARTFORGE_ARC_CICLOS": None,
}
expected = [(rep, mode) for rep in range(1, 8) for mode in
            (["antes", "depois"] if rep % 2 else ["depois", "antes"])]
assert [(row["repeticao"], row["modo"]) for row in rows] == expected
values = {mode: {name: [] for name in ["arvores", "lista_ligada"]} for mode in ["antes", "depois"]}
for row in rows:
    assert row["codigo"] == 0 and row["segundos_processo"] > 0
    matches = re.findall(r"^(\w+): ([\d ]+) us \| (.*)$", row["stdout"], re.MULTILINE)
    assert len(matches) == 2
    reconstructed = {}
    for name, raw_times, result in matches:
        assert name not in reconstructed
        times = [int(value) for value in raw_times.split()]
        assert len(times) == 6 and all(value > 0 for value in times)
        assert result == {"arvores": "3156655", "lista_ligada": "499999500000"}[name]
        median = statistics.median(times[1:])
        reconstructed[name] = {"rodadas_us": times, "mediana_us": median, "resultado": result}
        values[row["modo"]][name].append(median)
    assert reconstructed == row["nucleos"]
results = {}
for name in values["antes"]:
    before = statistics.median(values["antes"][name])
    after = statistics.median(values["depois"][name])
    results[name] = {"antes_us": before, "depois_us": after, "depois_antes": after / before,
                     "razoes_pareadas": [after / before for before, after in
                                          zip(values["antes"][name], values["depois"][name])]}
assert results == reported
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(Path(manifest["comando_compilacao"][0])) == manifest["cli_sha256"]
assert sha(out / "objetos_escapam-ARC.exe") == manifest["executaveis_sha256"]["depois"]
assert sha(root / "target/comparacao-arc-mortos-b706e0b0/objetos_escapam-ARC.exe") == manifest["executaveis_sha256"]["antes"]
for name, digest in manifest["entradas"].items():
    assert sha(root / "bench/desempenho" / name) == digest
    for revision in [manifest["fonte_antes"], manifest["fonte_depois"]]:
        raw = subprocess.check_output(["git", "show", revision + ":bench/desempenho/" + name], cwd=root)
        assert hashlib.sha256(raw).hexdigest() == digest
production = subprocess.check_output(["git", "diff", "--name-only", manifest["fonte_antes"],
                                      manifest["fonte_depois"], "--", "crates"], cwd=root, text=True)
assert production.splitlines() == ["crates/runtime/src/arc.rs"]
audit = {"estado": "ok", "execucoes": len(rows), "rodadas_por_execucao": 6,
         "aquecimento_descartado": 1, "pares": 7, "resultado_reconstruido": results,
         "depois_menor_por_kernel": {name: sum(ratio < 1 for ratio in result["razoes_pareadas"])
                                     for name, result in results.items()},
         "hashes_cli_executaveis_e_fontes_conferidos": True,
         "gate_completo_arc_a0": "nao_reavaliado"}
(out / "auditoria.json").write_text(json.dumps(audit, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps(audit, indent=2))
