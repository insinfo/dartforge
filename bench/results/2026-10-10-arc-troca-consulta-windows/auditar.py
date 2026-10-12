import hashlib
import json
from pathlib import Path
import re
import statistics

out = Path(__file__).resolve().parent
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
manifest = json.loads((out / 'manifesto.json').read_text(encoding='utf-8'))
result = json.loads((out / 'resultado.json').read_text(encoding='utf-8'))
rows = [json.loads(line) for line in (out / 'amostras.jsonl').read_text(encoding='utf-8').splitlines()]
assert len(rows) == 28
assert sha(out / 'mudanca.patch') == manifest['patch_sha256']
assert sha(out / 'cli-depois-nativo/dartforge.exe') == manifest['cli_depois_sha256']
assert sha(out / 'cli-antes-nativo/dartforge.exe') == manifest['referencia']['cli_antes_sha256']
for name, digest in manifest['executaveis'].items():
    assert sha(out / name) == digest
expected = {
    'objetos_escapam': {'arvores': '3156655', 'lista_ligada': '499999500000'},
    'objetos_em_colecoes': {'lista_objetos': '129592732.5', 'ordenar_objetos': '100028',
                           'mapa_int_objeto': '19999900000.0', 'mapa_str_objeto': '1199980'},
}
seen = set()
values = {}
memory = {}
for row in rows:
    rep, mode, program = row['repeticao'], row['modo'], row['programa']
    key = (rep, mode, program)
    assert key not in seen and 1 <= rep <= 7 and mode in ('antes', 'depois')
    seen.add(key)
    assert row['codigo'] == 0
    assert set(row['kernels']) == set(expected[program])
    parsed = {}
    for line in row['stdout'].splitlines():
        m = re.fullmatch(r'(\w+): ([0-9 ]+) us \| (.*)', line)
        assert m and m[1] not in parsed
        parsed[m[1]] = (list(map(int, m[2].split())), m[3])
    assert set(parsed) == set(expected[program])
    for kernel, sample in row['kernels'].items():
        times, answer = parsed[kernel]
        assert len(times) == 6 and all(t > 0 for t in times)
        assert answer == expected[program][kernel] == sample['resultado']
        median = statistics.median(times[1:])
        assert times == sample['rodadas_us'] and median == sample['mediana_us']
        values.setdefault(f'{program}/{kernel}', {}).setdefault(mode, {})[rep] = median
    for counter, peak in row['memoria'].items():
        assert peak > 0
        memory.setdefault(program, {}).setdefault(counter, {}).setdefault(mode, []).append(peak)
for name, modes in values.items():
    before = [modes['antes'][i] for i in range(1, 8)]
    after = [modes['depois'][i] for i in range(1, 8)]
    a, d = statistics.median(before), statistics.median(after)
    pairs = [y / x for x, y in zip(before, after)]
    assert result[name] == {'antes_us': a, 'depois_us': d, 'depois_antes': d / a, 'razoes_pareadas': pairs}
    result[name]['pares_mais_rapidos'] = sum(r < 1 for r in pairs)
    result[name]['reducao_percentual'] = 100 * (1 - d / a)
mem_results = {}
for program, counters in memory.items():
    mem_results[program] = {}
    for counter, modes in counters.items():
        a, d = statistics.median(modes['antes']), statistics.median(modes['depois'])
        mem_results[program][counter] = {'antes': a, 'depois': d, 'depois_antes': d / a}
audit = {'execucoes': len(rows), 'pares': 7, 'kernels': result, 'memoria': mem_results,
         'hashes_executaveis_conferidos': len(manifest['executaveis']),
         'resultados_esperados_conferidos': True, 'amostras_sha256': sha(out / 'amostras.jsonl')}
(out / 'auditoria.json').write_text(json.dumps(audit, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
print(json.dumps(audit, indent=2, ensure_ascii=False))
