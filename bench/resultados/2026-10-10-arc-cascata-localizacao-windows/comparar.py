"""Comparação pareada de uma alteração isolada no runtime ARC."""
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

root = Path(__file__).resolve().parents[2]
out = Path(__file__).resolve().parent
old = out / 'antes.exe'
new = out / 'depois.exe'
cli = root / 'target/release/dartforge.exe'
source = root / 'crates/runtime/src/arc.rs'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

assert sha(old) == '900a701131b49d02b0b8648125016ad8122bf6ceef5e211836bb2172de6b017f'
assert sha(cli) != sha(out / 'cli-antes/dartforge.exe'), 'CLI ainda é a versão de referência'
assert not (out / 'amostras.jsonl').exists(), 'não misturar rodadas'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == '7f91f6ae183b1439d020ccbd96173c248d0a9a9d'
assert subprocess.check_output(['git', 'diff', '--name-only', '--', 'crates'], cwd=root, text=True).splitlines() == ['crates/runtime/src/arc.rs']
patch = subprocess.check_output(['git', 'diff', '--binary', '--', 'crates/runtime/src/arc.rs'], cwd=root)
(out / 'mudanca.patch').write_bytes(patch)
env = os.environ.copy()
for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_CICLOS', 'DARTFORGE_ARC_TRACO'):
    env.pop(key, None)
env.update(DARTFORGE_RAIZES='sombra', DARTFORGE_GC_RASTRO='0', DARTFORGE_GC_OFF='0',
           DARTFORGE_ARC_CONFERIR='0', DARTFORGE_ARC_BERCARIO='0', DARTFORGE_HEAP_MAX_MB='256')
k = ctypes.windll.kernel32
k.GetCurrentProcess.restype = ctypes.c_void_p
assert k.SetProcessAffinityMask(ctypes.c_void_p(k.GetCurrentProcess()), ctypes.c_size_t(4))
class ContadoresMemoria(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in ('PeakWorkingSetSize', 'WorkingSetSize',
        'QuotaPeakPagedPoolUsage', 'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage',
        'QuotaNonPagedPoolUsage', 'PagefileUsage', 'PeakPagefileUsage')]
consultar_memoria = ctypes.WinDLL('psapi', use_last_error=True).GetProcessMemoryInfo
consultar_memoria.argtypes = [wintypes.HANDLE, ctypes.POINTER(ContadoresMemoria), wintypes.DWORD]
consultar_memoria.restype = wintypes.BOOL

def picos_memoria(process):
    counters = ContadoresMemoria()
    counters.cb = ctypes.sizeof(counters)
    assert consultar_memoria(int(process._handle), ctypes.byref(counters), counters.cb), ctypes.get_last_error()
    return {'working_set_pico_bytes': counters.PeakWorkingSetSize, 'commit_pico_bytes': counters.PeakPagefileUsage}
inputs = {name: sha(root / 'bench/desempenho' / name) for name in ('objetos_escapam.dart', 'comum.dart')}
command = [str(cli), 'aot', str(root / 'bench/desempenho/objetos_escapam.dart'), str(new),
           '--optimize', '--excecoes', 'checagem', '--memoria=arc']
manifest = {
    'base': '7f91f6ae183b1439d020ccbd96173c248d0a9a9d',
    'runtime_antes_blob': 'aa79d64b7d115a3f60d479162ba2ced60a824731',
    'runtime_depois_sha256': sha(source), 'patch_sha256': hashlib.sha256(patch).hexdigest(),
    'cli_antes_sha256': sha(out / 'cli-antes/dartforge.exe'), 'cli_depois_sha256': sha(cli),
    'entradas': inputs, 'afinidade': '0x4', 'repeticoes': 7, 'comando_compilacao': command,
    'medicao_memoria': 'GetProcessMemoryInfo depois do término; pico do processo inteiro, não por kernel',
    'ambiente': {key: env.get(key) for key in ('DARTFORGE_RAIZES', 'DARTFORGE_GC_RASTRO',
        'DARTFORGE_GC_OFF', 'DARTFORGE_ARC_CONFERIR', 'DARTFORGE_ARC_BERCARIO',
        'DARTFORGE_HEAP_MAX_MB', 'DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_CICLOS', 'DARTFORGE_ARC_TRACO')},
}
def save_manifest():
    (out / 'manifesto.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
save_manifest()
with (out / 'compilacao-depois.log').open('wb') as log:
    process = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
assert process.returncode == 0 and new.is_file(), 'falha na compilação'
bins = {'antes': old, 'depois': new}
hashes = {name: sha(path) for name, path in bins.items()}
manifest['executaveis_sha256'] = hashes
save_manifest()
print('AOT novo compilado; iniciando sete pares.', flush=True)
samples = {mode: {'arvores': [], 'lista_ligada': []} for mode in bins}
memory_samples = {mode: [] for mode in bins}
pattern = re.compile(r'^(\w+): ([\d ]+) us \| (.*)$', re.MULTILINE)
for repetition in range(1, 8):
    for mode in (['antes', 'depois'] if repetition % 2 else ['depois', 'antes']):
        start = time.perf_counter()
        with subprocess.Popen([str(bins[mode])], cwd=root / 'bench/desempenho', env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8') as p:
            try:
                stdout, stderr = p.communicate(timeout=1800)
            except subprocess.TimeoutExpired:
                p.kill()
                p.communicate()
                raise
            memory = picos_memoria(p)
            code = p.returncode
        row = {'repeticao': repetition, 'modo': mode, 'codigo': code,
               'stdout': stdout, 'stderr': stderr, 'segundos_processo': time.perf_counter() - start,
               'nucleos': {}, 'memoria': memory}
        for name, timings, result in pattern.findall(stdout):
            assert name not in row['nucleos']
            timings = list(map(int, timings.split()))
            assert len(timings) == 6
            row['nucleos'][name] = {'rodadas_us': timings, 'mediana_us': statistics.median(timings[1:]), 'resultado': result}
        with (out / 'amostras.jsonl').open('a', encoding='utf-8') as log:
            log.write(json.dumps(row, ensure_ascii=False) + '\n')
        assert code == 0 and {name: values['resultado'] for name, values in row['nucleos'].items()} == {
            'arvores': '3156655', 'lista_ligada': '499999500000'}
        for name, values in row['nucleos'].items():
            samples[mode][name].append(values['mediana_us'])
        memory_samples[mode].append(memory)
    print(f'Par {repetition}/7 concluído.', flush=True)
assert hashes == {name: sha(path) for name, path in bins.items()}
assert sha(cli) == manifest['cli_depois_sha256']
assert sha(source) == manifest['runtime_depois_sha256']
assert inputs == {name: sha(root / 'bench/desempenho' / name) for name in inputs}
results = {}
for name in samples['antes']:
    before, after = (statistics.median(samples[mode][name]) for mode in ('antes', 'depois'))
    results[name] = {'antes_us': before, 'depois_us': after, 'depois_antes': after / before,
        'razoes_pareadas': [d / a for a, d in zip(samples['antes'][name], samples['depois'][name])]}
(out / 'resultado.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')
print(json.dumps(results, indent=2), flush=True)
memory_result = {mode: {key: {'mediana_bytes': statistics.median(row[key] for row in rows),
                             'maximo_bytes': max(row[key] for row in rows)}
                        for key in ('working_set_pico_bytes', 'commit_pico_bytes')}
                 for mode, rows in memory_samples.items()}
(out / 'memoria.json').write_text(json.dumps(memory_result, indent=2) + '\n', encoding='utf-8')
print(json.dumps(memory_result, indent=2), flush=True)
