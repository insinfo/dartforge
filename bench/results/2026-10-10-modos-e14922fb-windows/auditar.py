"""Reconstrói o gate completo pelas amostras e pelo oráculo Dart fresco."""
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess

root = next(path for path in Path(__file__).resolve().parents if (path / 'Cargo.toml').is_file())
out = root / 'target/modos-2026-10-10-e14922fb-a0-arc-dart'
prov = json.loads((out / 'proveniencia.json').read_text(encoding='utf-8'))
rows = [json.loads(line) for line in (out / 'amostras.jsonl').read_text(encoding='utf-8').splitlines()]
programs = sorted(path.stem for path in (root / 'bench/desempenho').glob('*.dart') if path.stem != 'comum')
modes = ['A0', 'ARC', 'dart']
assert rows[0] == {'tipo': 'configuracao', 'programas': programs, 'modos': modes, 'repeticoes': 7, 'afinidade': '0x4'}
assert len(programs) == 9 and len(rows[1:]) == 189
sequence = [(program, mode, repetition + 1) for repetition in range(7) for program in programs
            for mode in modes[repetition % 3:] + modes[:repetition % 3]]
assert [(row['programa'], row['modo'], row['repeticao']) for row in rows[1:]] == sequence
previous = json.loads((root / 'bench/resultados/2026-10-10-modos-3189c762-windows/auditoria.json').read_text(encoding='utf-8'))
expected_kernels = {}
for key in previous['razoes']:
    program, kernel = key.split('/')
    expected_kernels.setdefault(program, set()).add(kernel)
results, gold, measurements = {}, {}, {}
pattern = re.compile(r'^(\w+): ([\d ]+) us \| (.*)$')
expected_extra = {
    'colecoes': {'ordenacao conferida: -9223372036854775808 9223372036854775807'},
    'json': {'bytes: 103917, true', 'decodificado: 819486632',
             'grande: 5974162 unidades, soma 980022366, ida e volta true',
             'indentado: 179169 unidades, soma 661525918, ida e volta true',
             'medio: 101447 unidades, soma 642823872, ida e volta true',
             'pequeno: 1080 unidades, soma 319775962, ida e volta true'},
}
for row in rows[1:]:
    assert row['tipo'] == 'execucao' and row['codigo'] == 0
    assert row['stderr'] == '', 'rastro/diagnóstico inesperado durante a medição'
    parsed = {}
    extra = []
    for line in row['stdout'].splitlines():
        match = pattern.fullmatch(line.strip())
        if not match:
            extra.append(line.strip())
            continue
        name, raw, result = match.groups()
        assert name not in parsed
        times = [int(value) for value in raw.split()]
        rounds = 4 if row['programa'] == 'json' and name in ['decode_grande', 'encode_grande'] else 6
        assert len(times) == rounds and all(value > 0 for value in times)
        median = statistics.median(times[1:])
        parsed[name] = [median, result]
        key = row['programa'], name
        assert results.setdefault(key, result) == result
        if row['modo'] == 'dart':
            assert gold.setdefault(key, result) == result
        measurements.setdefault((row['programa'], row['modo'], name), []).append(median)
    assert parsed == row['nucleos']
    assert set(parsed) == expected_kernels[row['programa']]
    assert len(extra) == len(expected_extra.get(row['programa'], set()))
    assert set(extra) == expected_extra.get(row['programa'], set())
assert results == gold and len(gold) == 32
ratios = {}
for program, names in expected_kernels.items():
    for name in sorted(names):
        values = {mode: measurements[program, mode, name] for mode in modes}
        assert all(len(items) == 7 for items in values.values())
        medians = {mode: statistics.median(items) for mode, items in values.items()}
        ratios[program + '/' + name] = {'ARC_us': medians['ARC'], 'A0_us': medians['A0'],
            'Dart_us': medians['dart'], 'ARC_A0': medians['ARC'] / medians['A0'],
            'razoes_pareadas_ARC_A0': [a / b for a, b in zip(values['ARC'], values['A0'])]}

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

for name, digest in prov['entradas'].items():
    assert sha(root / 'bench/desempenho' / name) == digest
    raw = subprocess.check_output(['git', 'show', prov['fonte'] + ':bench/desempenho/' + name], cwd=root)
    assert hashlib.sha256(raw).hexdigest() == digest
assert sha(out / 'cli-produtora/dartforge.exe') == prov['cli_sha256']
assert sha(root / 'scripts/medir-modos-desempenho.py') == prov['script_sha256']
assert sha(Path('C:/tools/dartsdk-3.6.2/bin/dart.exe')) == prov['dart_sha256']
assert subprocess.check_output(['git', 'rev-parse', prov['fonte'] + ':crates'], cwd=root, text=True).strip() == prov['arvore_crates']
fingerprints = json.loads((out / 'executaveis-acompanhamento.json').read_text(encoding='utf-8'))
files = fingerprints['executaveis']
assert len(files) == 27 and {record['nome'] for record in files} == {
    program + '-' + mode + '.exe' for program in programs for mode in modes}
pre_execution = 0
for record in files:
    assert record['compilacao_confirmada_no_log']
    assert sha(out / record['nome']) == record['sha256']
    first = next(i for i, (program, mode, _) in enumerate(sequence)
                 if record['nome'] == program + '-' + mode + '.exe')
    # O processo atual pode já ter começado sem registro de conclusão.
    # A desigualdade estrita evita tratar seu hash tardio como anterior.
    if not record['fase2_iniciada'] or record['execucoes_concluidas_no_snapshot'] < first:
        pre_execution += 1
geo = statistics.geometric_mean(value['ARC_A0'] for value in ratios.values())
audit = {'fonte': prov['fonte'], 'execucoes': len(rows) - 1, 'programas': len(programs), 'kernels': len(ratios),
    'resultados_iguais_ao_dart_fresco': True, 'saidas_adicionais_conferidas': True, 'aquecimento_descartado': 1,
    'rodadas_por_kernel': {key: 4 if key in ['json/decode_grande', 'json/encode_grande'] else 6 for key in ratios},
    'hashes_conferidos': len(files), 'hashes_comprovadamente_antes_primeira_execucao': pre_execution,
    'snapshot_hashes': 'Durante a rodada; não comprova identidade anterior à primeira execução dos demais arquivos.',
    'geo_ARC_A0': geo, 'gate_ARC_A0': 'FAIL' if geo > 1 else 'avaliar_demais_criterios', 'razoes': ratios}
(out / 'auditoria.json').write_text(json.dumps(audit, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({key: value for key, value in audit.items() if key != 'razoes'}, indent=2, ensure_ascii=False))
