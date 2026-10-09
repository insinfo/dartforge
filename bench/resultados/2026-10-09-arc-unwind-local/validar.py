import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

raiz = next(p for p in Path(__file__).resolve().parents if (p / 'Cargo.toml').is_file())
saida = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else raiz / 'target' / 'arc-unwind-validacao'
saida.mkdir(exist_ok=False)
cli = raiz / 'target' / 'release' / 'dartforge.exe'
dart = Path(os.environ['DART_SDK']) / 'bin' / 'dart.exe'
registros = []
def hash_arquivo(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def executar(nome, comando, ambiente, limite):
    inicio = time.monotonic()
    try:
        p = subprocess.run([str(x) for x in comando], cwd=raiz, env=ambiente,
                           capture_output=True, text=True, timeout=limite)
        registro = dict(nome=nome, comando=[str(x) for x in comando], codigo=p.returncode,
                        stdout=p.stdout, stderr=p.stderr, segundos=time.monotonic()-inicio)
    except subprocess.TimeoutExpired:
        registro = dict(nome=nome, comando=[str(x) for x in comando], timeout=limite,
                        segundos=time.monotonic()-inicio)
    registros.append(registro)
    (saida / 'resultados.json').write_text(json.dumps(registros, ensure_ascii=False, indent=2), encoding='utf-8')
    print(nome, registro.get('codigo', 'timeout'), flush=True)
    if registro.get('codigo') != 0:
        raise RuntimeError(json.dumps(registro, ensure_ascii=False))
    return registro['stdout']

ambiente = os.environ.copy()
for chave in ['DARTFORGE_GC_STRESS', 'DARTFORGE_SDK_DLL', 'DARTFORGE_GC_SABOTAGEM']:
    ambiente.pop(chave, None)
ambiente.update(DARTFORGE_GC_OFF='0', DARTFORGE_GC_RASTRO='0',
                DARTFORGE_ARC_CONFERIR='1', DARTFORGE_ARC_CICLOS='sempre',
                DARTFORGE_ARC_BERCARIO='0', DARTFORGE_HEAP_MAX_MB='256',
                DARTFORGE_SDK_DA_FONTE='1')
hash_cli = hash_arquivo(cli)
(saida / 'cli.json').write_text(json.dumps(dict(sha256=hash_cli,
    fonte=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=raiz, text=True).strip(),
    script_sha256=hash_arquivo(Path(__file__))), indent=2), encoding='utf-8')
executaveis = []
for caso, estresse in [('gc_d03_excecao_profunda', False), ('gc_d04_finally', True)]:
    fonte = raiz / 'corpus' / 'nativo' / (caso + '.dart')
    esperado = executar(caso + '-dart', [dart, fonte], ambiente, 120)
    for memoria in ['tracing', 'arc']:
        for excecoes in ['checagem', 'tabelas']:
            nome = f'{caso}-{memoria}-{excecoes}'
            exe = saida / (nome + '.exe')
            executar(nome + '-compilar', [cli, 'aot', fonte, exe, '--optimize',
                     '--memoria', memoria, '--excecoes', excecoes], ambiente, 180)
            exec_env = ambiente.copy()
            if estresse:
                exec_env['DARTFORGE_GC_STRESS'] = '1'
            obtido = executar(nome + '-executar', [exe], exec_env, 120)
            executaveis.append(dict(nome=nome, sha256=hash_arquivo(exe),
                                    gc_stress=estresse, saida_igual_dart=obtido == esperado))
            (saida / 'executaveis.json').write_text(json.dumps(executaveis, indent=2), encoding='utf-8')
            if obtido != esperado:
                raise AssertionError(f'{nome}: stdout diferente do Dart')
assert hash_arquivo(cli) == hash_cli
print('18 processos com código zero; oito saídas AOT iguais ao Dart.', flush=True)
