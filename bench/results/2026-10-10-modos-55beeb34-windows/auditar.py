"""Fecha a medição por dados brutos, cobertura, resultados e hashes."""
import hashlib
import json
import re
import statistics
import subprocess
from pathlib import Path

root = next(p for p in Path(__file__).resolve().parents if (p / "Cargo.toml").is_file() and (p / "CONTRIBUTING.md").is_file())
out = root / "target/modos-2026-10-10-55beeb34-a0-arc"
proveniencia = json.loads((root / "target/modos-55beeb34-proveniencia.json").read_text(encoding="utf-8"))
rows = [json.loads(l) for l in (out / "amostras.jsonl").read_text(encoding="utf-8").splitlines()]
config = rows[0]
assert config["tipo"] == "configuracao" and config["repeticoes"] == 7
assert config["modos"] == ["A0", "ARC"] and config["afinidade"] == "0x4"
assert len(config["programas"]) == 9 and len(rows[1:]) == 126
expected = {(p, m, r) for p in config["programas"] for m in config["modos"] for r in range(1, 8)}
sequence = [(p, m, r) for r in range(1, 8) for p in config["programas"]
            for m in (["A0", "ARC"] if r % 2 else ["ARC", "A0"])]
assert [(r["programa"], r["modo"], r["repeticao"]) for r in rows[1:]] == sequence
seen, results, measurements, kernels = set(), {}, {}, {}
pattern = re.compile(r"^(\w+): ([\d ]+) us \| (.*)$")
for row in rows[1:]:
    assert row["tipo"] == "execucao" and row["codigo"] == 0
    key = row["programa"], row["modo"], row["repeticao"]
    assert key in expected and key not in seen
    seen.add(key)
    parsed = {}
    for line in row["stdout"].splitlines():
        match = pattern.match(line.strip())
        if not match:
            continue
        name, times, result = match.groups()
        assert name not in parsed
        times = [int(t) for t in times.split()]
        assert len(times) > 1
        parsed[name] = [statistics.median(times[1:]), result]
    assert parsed == row["nucleos"] and parsed
    assert kernels.setdefault(key[0], set(parsed)) == set(parsed)
    for name, (time, result) in parsed.items():
        assert results.setdefault((key[0], name), result) == result
        measurements.setdefault((key[0], key[1], name), []).append(time)
assert seen == expected and sum(map(len, kernels.values())) == 32
for a in proveniencia["fontes_benchmark"]:
    data = (root / "bench/desempenho" / a["nome"]).read_bytes()
    assert hashlib.sha256(data).hexdigest() == a["sha256"]
    previous = subprocess.check_output(["git", "show", "5bbfc80f:bench/desempenho/" + a["nome"]], cwd=root)
    assert hashlib.sha256(previous).hexdigest() == a["sha256"]
gold_path = root / "bench/resultados/2026-10-09-modos-faixas-windows/amostras.jsonl"
gold = {}
for line in gold_path.read_text(encoding="utf-8").splitlines():
    row = json.loads(line)
    if row.get("tipo") == "execucao" and row["modo"] == "dart":
        assert row["codigo"] == 0
        for name, (_, result) in row["nucleos"].items():
            assert gold.setdefault((row["programa"], name), result) == result
assert results == gold
ratios = {}
for p, names in kernels.items():
    for n in names:
        arc, a0 = (measurements[(p, m, n)] for m in ["ARC", "A0"])
        assert len(arc) == len(a0) == 7
        ta, t0 = statistics.median(arc), statistics.median(a0)
        assert ta > 0 and t0 > 0
        ratios[p + "/" + n] = {"ARC_us": ta, "A0_us": t0, "ARC_A0": ta / t0}
fingerprints = json.loads((out / "executaveis-inicio.json").read_text(encoding="utf-8"))
assert len(fingerprints) == 18
for a in fingerprints:
    assert hashlib.sha256((out / a["nome"]).read_bytes()).hexdigest() == a["sha256"]
audit = {"fonte": proveniencia["fonte"], "execucoes": len(seen), "programas": len(kernels),
         "kernels": len(ratios), "resultados_iguais_ao_dart_anterior": True,
         "executaveis_inalterados": 18, "geo_ARC_A0": statistics.geometric_mean(v["ARC_A0"] for v in ratios.values()),
         "razoes": ratios, "gold_sha256": hashlib.sha256(gold_path.read_bytes()).hexdigest()}
audit["gate_ARC_A0"] = "FAIL" if audit["geo_ARC_A0"] > 1 else "avaliar_demais_criterios"
(out / "auditoria.json").write_text(json.dumps(audit, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print({k: v for k, v in audit.items() if k != "razoes"})
