"""Reconstrói o gate completo a partir das amostras, sem usar o relatório agregado."""
import hashlib, json, re, statistics, subprocess
from pathlib import Path
root = next(p for p in Path(__file__).resolve().parents if (p / "Cargo.toml").is_file())
out = root / "target/modos-2026-10-10-3189c762-a0-arc-dart"
prov = json.loads((out / "proveniencia.json").read_text(encoding="utf-8"))
rows = [json.loads(l) for l in (out / "amostras.jsonl").read_text(encoding="utf-8").splitlines()]
config = rows[0]
programs = sorted(p.stem for p in (root / "bench/desempenho").glob("*.dart") if p.stem != "comum")
assert config == {"tipo": "configuracao", "programas": programs, "modos": ["A0", "ARC", "dart"], "repeticoes": 7, "afinidade": "0x4"}
assert len(programs) == 9 and len(rows[1:]) == 189
modes = config["modos"]
sequence = [(p, m, r + 1) for r in range(7) for p in programs for m in modes[r % 3:] + modes[:r % 3]]
assert [(r["programa"], r["modo"], r["repeticao"]) for r in rows[1:]] == sequence
old = json.loads((root / "bench/resultados/2026-10-10-modos-55beeb34-windows/auditoria.json").read_text(encoding="utf-8"))
expected_kernels = {}
for key in old["razoes"]:
    p, n = key.split("/")
    expected_kernels.setdefault(p, set()).add(n)
results, gold, measurements = {}, {}, {}
pattern = re.compile(r"^(\w+): ([\d ]+) us \| (.*)$")
for row in rows[1:]:
    assert row["tipo"] == "execucao" and row["codigo"] == 0
    parsed = {}
    for line in row["stdout"].splitlines():
        match = pattern.match(line.strip())
        if not match:
            continue
        name, raw, result = match.groups()
        assert name not in parsed
        times = [int(v) for v in raw.split()]
        # Os dois kernels JSON grandes pedem quatro rodadas na fonte;
        # os demais usam as seis do medir(), sem aceitar truncamento.
        rounds = 4 if row["programa"] == "json" and name in ["decode_grande", "encode_grande"] else 6
        assert len(times) == rounds and all(v > 0 for v in times)
        median = statistics.median(times[1:])
        parsed[name] = [median, result]
        key = row["programa"], name
        assert results.setdefault(key, result) == result
        if row["modo"] == "dart":
            assert gold.setdefault(key, result) == result
        measurements.setdefault((row["programa"], row["modo"], name), []).append(median)
    assert parsed == row["nucleos"]
    assert set(parsed) == expected_kernels[row["programa"]]
assert results == gold and len(gold) == 32
ratios = {}
for p, names in expected_kernels.items():
    for n in sorted(names):
        values = {m: measurements[p, m, n] for m in modes}
        assert all(len(v) == 7 for v in values.values())
        medians = {m: statistics.median(v) for m, v in values.items()}
        ratios[p + "/" + n] = {"ARC_us": medians["ARC"], "A0_us": medians["A0"],
            "Dart_us": medians["dart"], "ARC_A0": medians["ARC"] / medians["A0"],
            "razoes_pareadas_ARC_A0": [a / b for a, b in zip(values["ARC"], values["A0"])]}
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
for name, digest in prov["entradas"].items():
    assert sha(root / "bench/desempenho" / name) == digest
    raw = subprocess.check_output(["git", "show", prov["produtor_cli"] + ":bench/desempenho/" + name], cwd=root)
    assert hashlib.sha256(raw).hexdigest() == digest
assert sha(out / prov["cli_preservada"]) == prov["cli_sha256"]
assert sha(root / "scripts/medir-modos-desempenho.py") == prov["script_medicao_sha256"]
assert subprocess.check_output(["git", "rev-parse", prov["fonte"] + ":crates"], cwd=root, text=True).strip() == prov["arvore_crates"]
assert subprocess.check_output(["git", "rev-parse", prov["produtor_cli"] + ":crates"], cwd=root, text=True).strip() == prov["arvore_crates"]
fingerprints = json.loads((out / "executaveis-acompanhamento.json").read_text(encoding="utf-8"))
files = fingerprints["executaveis"]
assert {f["nome"] for f in files} == {p + "-" + m + ".exe" for p in programs for m in modes}
for f in files:
    assert sha(out / f["nome"]) == f["sha256"]
geo = statistics.geometric_mean(r["ARC_A0"] for r in ratios.values())
audit = {"fonte": prov["fonte"], "produtor_cli": prov["produtor_cli"], "execucoes": len(rows) - 1,
    "programas": len(programs), "kernels": len(ratios), "resultados_iguais_ao_dart_fresco": True,
    "rodadas_por_kernel": {key: 4 if key in ["json/decode_grande", "json/encode_grande"] else 6 for key in ratios},
    "aquecimento_descartado": 1,
    "hashes_conferidos": 27, "snapshot_hashes": "durante a rodada, não anterior a todas as execuções",
    "linhas_amostras_no_snapshot": fingerprints["linhas_amostras_no_snapshot"],
    "geo_ARC_A0": geo, "gate_ARC_A0": "FAIL" if geo > 1 else "avaliar_demais_criterios",
    "razoes": ratios}
(out / "auditoria.json").write_text(json.dumps(audit, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
print(json.dumps({k: v for k, v in audit.items() if k != "razoes"}, indent=2, ensure_ascii=False))
