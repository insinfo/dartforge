"""Reconstrói a comparação somente a partir dos registros brutos."""
import hashlib
import json
from pathlib import Path
import re
import statistics

out = Path(__file__).resolve().parent
manifest = json.loads((out / 'manifesto.json').read_text(encoding='utf-8'))
rows = [json.loads(line) for line in (out / 'amostras.jsonl').read_text(encoding='utf-8').splitlines()]
expected_order = [(rep, mode) for rep in range(1, 8)
                  for mode in (['antes', 'depois'] if rep % 2 else ['depois', 'antes'])]
assert [(row['repeticao'], row['modo']) for row in rows] == expected_order
assert manifest['repeticoes'] == 7 and manifest['afinidade'] == '0x4'
assert hashlib.sha256((out / 'mudanca.patch').read_bytes()).hexdigest() == manifest['patch_sha256']
samples = {mode: {'arvores': [], 'lista_ligada': []} for mode in ('antes', 'depois')}
memories = {mode: [] for mode in samples}
expected_results = {'arvores': '3156655', 'lista_ligada': '499999500000'}
for row in rows:
    assert row['codigo'] == 0 and row['stderr'] == ''
    parsed = {}
    for line in row['stdout'].splitlines():
        match = re.fullmatch(r'(\w+): ([\d ]+) us \| (.*)', line)
        assert match and match[1] not in parsed
        timings = [int(value) for value in match[2].split()]
        assert len(timings) == 6 and all(value > 0 for value in timings)
        parsed[match[1]] = {'rodadas_us': timings, 'mediana_us': statistics.median(timings[1:]), 'resultado': match[3]}
    assert {name: values['resultado'] for name, values in parsed.items()} == expected_results
    assert parsed == row['nucleos']
    for name, values in parsed.items():
        samples[row['modo']][name].append(values['mediana_us'])
    assert set(row['memoria']) == {'working_set_pico_bytes', 'commit_pico_bytes'}
    assert all(isinstance(value, int) and value > 0 for value in row['memoria'].values())
    memories[row['modo']].append(row['memoria'])
reconstructed = {}
for name in expected_results:
    before = statistics.median(samples['antes'][name])
    after = statistics.median(samples['depois'][name])
    reconstructed[name] = {'antes_us': before, 'depois_us': after, 'depois_antes': after / before,
        'razoes_pareadas': [d / a for a, d in zip(samples['antes'][name], samples['depois'][name])]}
assert reconstructed == json.loads((out / 'resultado.json').read_text(encoding='utf-8'))
memory_reconstructed = {mode: {key: {'mediana_bytes': statistics.median(row[key] for row in values),
                                   'maximo_bytes': max(row[key] for row in values)}
                              for key in ('working_set_pico_bytes', 'commit_pico_bytes')}
                        for mode, values in memories.items()}
assert memory_reconstructed == json.loads((out / 'memoria.json').read_text(encoding='utf-8'))
audit = {'processos_conferidos': len(rows), 'ordem_pareada': True,
         'resultados_iguais': True, 'medianas_reconstruidas': True,
         'picos_memoria_reconstruidos': True, 'patch_conferido': True,
         'pares_mais_rapidos': {name: sum(value < 1 for value in values['razoes_pareadas'])
                                for name, values in reconstructed.items()},
         'limite': 'Dois kernels; não comprova o gate ARC/A0 nem significância estatística.'}
(out / 'auditoria.json').write_text(json.dumps(audit, indent=2) + '\n', encoding='utf-8')
print(json.dumps(audit, indent=2))
