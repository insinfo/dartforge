import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

out = Path(__file__).resolve().parent
root = out.parents[1]
source = root / 'crates/runtime/src/heap.rs'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
base = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
assert base == '8e77d2c07c477b2f4981dc0a5f5330a8265f97a8'
assert not subprocess.check_output(['git', 'diff', '--name-only', '--', 'crates'], cwd=root, text=True).strip()
saved = out / 'cli-antes-nativo/dartforge.exe'
saved.parent.mkdir(exist_ok=True)
assert not saved.exists()
shutil.copy2(root / 'target/release/dartforge.exe', saved)
data = {'base': base, 'runtime_antes_sha256': sha(source), 'cli_antes_sha256': sha(saved),
        'programas': {}, 'entradas': {'comum.dart': sha(root / 'bench/desempenho/comum.dart')}}
env = os.environ.copy()
for key in ('DARTFORGE_GC_STRESS', 'DARTFORGE_ARC_CICLOS', 'DARTFORGE_ARC_TRACO'):
    env.pop(key, None)
env.update(DARTFORGE_RAIZES='sombra', DARTFORGE_GC_RASTRO='0', DARTFORGE_GC_OFF='0',
           DARTFORGE_ARC_CONFERIR='0', DARTFORGE_ARC_BERCARIO='0', DARTFORGE_HEAP_MAX_MB='256')
for name in ('objetos_escapam', 'objetos_em_colecoes'):
    src = root / f'bench/desempenho/{name}.dart'
    exe = out / f'{name}-antes.exe'
    cmd = [str(saved), 'aot', str(src), str(exe), '--optimize', '--excecoes', 'checagem', '--memoria=arc']
    assert not exe.exists()
    with (out / f'{name}-antes-compilacao.log').open('wb') as log:
        p = subprocess.run(cmd, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    assert p.returncode == 0 and exe.is_file()
    data['programas'][name] = {'comando': cmd, 'executavel_sha256': sha(exe)}
    data['entradas'][src.name] = sha(src)
    print(f'Referência compilada: {name}', flush=True)
assert sha(source) == data['runtime_antes_sha256']
(out / 'referencia.json').write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8', newline='\n')
print('Referência preservada.', flush=True)
