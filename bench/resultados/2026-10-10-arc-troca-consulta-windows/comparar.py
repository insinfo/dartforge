import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess

out = Path(__file__).resolve().parent
root = out.parents[1]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
save = lambda p, v: p.write_text(json.dumps(v, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
base = json.loads((out / 'referencia.json').read_text(encoding='utf-8'))
source = root / 'crates/runtime/src/heap.rs'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip() == base['base']
assert subprocess.check_output(['git', 'diff', '--name-only', '--', 'crates'], cwd=root, text=True).splitlines() == ['crates/runtime/src/heap.rs']
assert not (out / 'amostras.jsonl').exists()
patch = subprocess.check_output(['git', 'diff', '--binary', '--', 'crates/runtime/src/heap.rs'], cwd=root)
(out / 'mudanca.patch').write_bytes(patch)
cli = out / 'cli-depois-nativo/dartforge.exe'
cli.parent.mkdir(exist_ok=True)
assert not cli.exists()
shutil.copy2(root / 'target/release/dartforge.exe', cli)
assert sha(cli) != base['cli_antes_sha256']
assert sha(source) != base['runtime_antes_sha256']
env = os.environ.copy()
for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_CICLOS', 'DARTFORGE_ARC_TRACO'):
    env.pop(key, None)
env.update(DARTFORGE_RAIZES='sombra', DARTFORGE_GC_RASTRO='0', DARTFORGE_GC_OFF='0',
           DARTFORGE_ARC_CONFERIR='0', DARTFORGE_ARC_BERCARIO='0', DARTFORGE_HEAP_MAX_MB='256')
expected = {
    'objetos_escapam': {'arvores': '3156655', 'lista_ligada': '499999500000'},
    'objetos_em_colecoes': {'lista_objetos': '129592732.5', 'ordenar_objetos': '100028',
                           'mapa_int_objeto': '19999900000.0', 'mapa_str_objeto': '1199980'},
}
manifest = {'referencia': base, 'runtime_depois_sha256': sha(source), 'cli_depois_sha256': sha(cli),
            'patch_sha256': hashlib.sha256(patch).hexdigest(), 'executaveis': {}, 'comandos': {},
            'afinidade': '0x4', 'repeticoes': 7, 'ambiente': {key: env.get(key) for key in (
                'DARTFORGE_RAIZES', 'DARTFORGE_GC_RASTRO', 'DARTFORGE_GC_OFF', 'DARTFORGE_ARC_CONFERIR',
                'DARTFORGE_ARC_BERCARIO', 'DARTFORGE_HEAP_MAX_MB', 'DARTFORGE_GC_STRESS',
                'DARTFORGE_ARC_CICLOS', 'DARTFORGE_ARC_TRACO')}}
for name in expected:
    exe = out / f'{name}-depois.exe'
    cmd = [str(cli), 'aot', str(root / f'bench/desempenho/{name}.dart'), str(exe),
           '--optimize', '--excecoes', 'checagem', '--memoria=arc']
    assert not exe.exists()
    with (out / f'{name}-depois-compilacao.log').open('wb') as log:
        p = subprocess.run(cmd, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    assert p.returncode == 0 and exe.is_file()
    manifest['comandos'][name] = cmd
    for mode in ('antes', 'depois'):
        path = out / f'{name}-{mode}.exe'
        manifest['executaveis'][path.name] = sha(path)
        if mode == 'antes':
            assert sha(path) == base['programas'][name]['executavel_sha256']
for name, digest in base['entradas'].items():
    assert sha(root / f'bench/desempenho/{name}') == digest
save(out / 'manifesto.json', manifest)
k = ctypes.WinDLL('kernel32', use_last_error=True)
k.GetCurrentProcess.restype = wintypes.HANDLE
k.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
assert k.SetProcessAffinityMask(k.GetCurrentProcess(), 4)
class Memory(ctypes.Structure):
    _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD)] + [
        (n, ctypes.c_size_t) for n in ('PeakWorkingSetSize', 'WorkingSetSize', 'QuotaPeakPagedPoolUsage',
        'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage', 'QuotaNonPagedPoolUsage', 'PagefileUsage', 'PeakPagefileUsage')]
mem = ctypes.WinDLL('psapi', use_last_error=True).GetProcessMemoryInfo
mem.argtypes = [wintypes.HANDLE, ctypes.POINTER(Memory), wintypes.DWORD]
pattern = re.compile(r'^(\w+): ([\d ]+) us \| (.*)$')
samples = {}
for repetition in range(1, 8):
    for mode in (('antes', 'depois') if repetition % 2 else ('depois', 'antes')):
        for name in expected:
            with subprocess.Popen([str(out / f'{name}-{mode}.exe')], cwd=root / 'bench/desempenho',
                    env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8') as p:
                stdout, stderr = p.communicate()
                counters = Memory()
                counters.cb = ctypes.sizeof(counters)
                assert mem(int(p._handle), ctypes.byref(counters), counters.cb)
                row = {'repeticao': repetition, 'modo': mode, 'programa': name, 'codigo': p.returncode,
                       'stdout': stdout, 'stderr': stderr, 'kernels': {},
                       'memoria': {'working_set_pico_bytes': counters.PeakWorkingSetSize,
                                   'commit_pico_bytes': counters.PeakPagefileUsage}}
            for line in stdout.splitlines():
                match = pattern.fullmatch(line)
                assert match, line
                kernel, times, result = match.groups()
                times = list(map(int, times.split()))
                assert len(times) == 6 and kernel not in row['kernels']
                row['kernels'][kernel] = {'rodadas_us': times, 'mediana_us': statistics.median(times[1:]), 'resultado': result}
            with (out / 'amostras.jsonl').open('a', encoding='utf-8', newline='\n') as log:
                log.write(json.dumps(row, ensure_ascii=False) + '\n')
            assert row['codigo'] == 0
            assert {kernel: v['resultado'] for kernel, v in row['kernels'].items()} == expected[name]
            for kernel, v in row['kernels'].items():
                samples.setdefault(f'{name}/{kernel}', {'antes': [], 'depois': []})[mode].append(v['mediana_us'])
    print(f'Par {repetition}/7 concluído.', flush=True)
results = {}
for name, values in samples.items():
    a, d = statistics.median(values['antes']), statistics.median(values['depois'])
    results[name] = {'antes_us': a, 'depois_us': d, 'depois_antes': d / a,
                     'razoes_pareadas': [after / before for before, after in zip(values['antes'], values['depois'])]}
for name, digest in manifest['executaveis'].items():
    assert sha(out / name) == digest
assert sha(cli) == manifest['cli_depois_sha256'] and sha(source) == manifest['runtime_depois_sha256']
save(out / 'resultado.json', results)
print(json.dumps(results, indent=2), flush=True)
