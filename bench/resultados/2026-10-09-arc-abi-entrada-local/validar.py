"""Valida a entrada ARC real e seu ramo fatal com um runtime incompatível."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

raiz = next(p for p in Path(__file__).resolve().parents if (p / 'Cargo.toml').is_file())
saida = Path(sys.argv[1]) if len(sys.argv) > 1 else raiz / 'target/arc-abi-entrada-validacao'
saida.mkdir(parents=True, exist_ok=False)
cli = raiz / 'target/release/dartforge.exe'
fonte = raiz / 'corpus/nativo/gc_d04_finally.dart'
clang = shutil.which('clang')
dart = shutil.which('dart')
assert clang and dart
if os.name == 'nt':
    ctypes.windll.kernel32.SetErrorMode(0x8003)

def hash_arquivo(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

ambiente = os.environ.copy()
for nome in ['DARTFORGE_SDK_DLL', 'DARTFORGE_GC_SABOTAGEM', 'DARTFORGE_GC_STRESS']:
    ambiente.pop(nome, None)
ambiente.update(DARTFORGE_SDK_DA_FONTE='1', DARTFORGE_ARC_CONFERIR='1',
    DARTFORGE_ARC_CICLOS='sempre', DARTFORGE_ARC_BERCARIO='0',
    DARTFORGE_GC_OFF='0', DARTFORGE_GC_RASTRO='0', DARTFORGE_HEAP_MAX_MB='256')
resultados = []
imagens = []
hash_cli = hash_arquivo(cli)
(saida / 'cli.json').write_text(json.dumps(dict(sha256=hash_cli,
    fonte=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=raiz, text=True).strip(),
    entrada_sha256=hash_arquivo(fonte), script_sha256=hash_arquivo(__file__),
    clang=clang, dart=dart, ambiente={k: ambiente.get(k) for k in [
        'DARTFORGE_SDK_DA_FONTE', 'DARTFORGE_SDK_DLL', 'DARTFORGE_ARC_CONFERIR',
        'DARTFORGE_ARC_CICLOS', 'DARTFORGE_ARC_BERCARIO', 'DARTFORGE_GC_OFF',
        'DARTFORGE_GC_RASTRO', 'DARTFORGE_GC_STRESS', 'DARTFORGE_GC_SABOTAGEM',
        'DARTFORGE_HEAP_MAX_MB']}), indent=2), encoding='utf-8')

def executar(nome, args, env=ambiente, sucesso=True, limite=180):
    inicio = time.perf_counter()
    p = subprocess.run([str(a) for a in args], cwd=raiz, env=env,
                       capture_output=True, text=True, timeout=limite)
    resultados.append(dict(nome=nome, args=[str(a) for a in args],
        segundos=time.perf_counter() - inicio, codigo=p.returncode,
        sucesso_esperado=sucesso, stdout=p.stdout, stderr=p.stderr))
    (saida / 'resultados.json').write_text(json.dumps(resultados, indent=2), encoding='utf-8')
    print(f'{nome}: codigo {p.returncode}', flush=True)
    assert (p.returncode == 0) == sucesso, nome
    return p.stdout

esperado = executar('dart', [dart, fonte])
ir = saida / 'entrada-real.ll'
executar('emitir-ir', [cli, 'compile-native', fonte, '--emit-ir', '-o', ir,
                      '--optimize', '--memoria', 'arc'])
texto = ir.read_text(encoding='utf-8')
guarda = None
for nome_funcao in ['df.preparar_isolado', 'dartforge_entry']:
    inicio = texto.find(f'define void @{nome_funcao}() {{')
    if inicio < 0:
        continue
    fim_funcao = texto.index('\n}', inicio)
    if '@dartforge_arc_verificar_abi(i64 1)' not in texto[inicio:fim_funcao]:
        continue
    inicio = texto.index('{', inicio) + 1
    fim = texto.index('df.arc.compativel:\n', inicio) + len('df.arc.compativel:\n')
    guarda = texto[inicio:fim]
    break
assert guarda is not None, 'entrada ARC sem guarda'
assert '@dartforge_arc_verificar_abi(i64 1)' in guarda
assert 'call void @llvm.trap()' in guarda
(saida / 'guarda-extraida.ll').write_text(guarda, encoding='utf-8')

for compatibilidade in [0, 1]:
    nome = f'guarda-runtime-{compatibilidade}'
    trecho = saida / (nome + '.ll')
    exe = saida / (nome + '.exe')
    trecho.write_text('declare void @llvm.trap()\n'
        'define void @dartforge_memoria_arc_v1() { ret void }\n'
        f'define i8 @dartforge_arc_verificar_abi(i64 %v) {{ ret i8 {compatibilidade} }}\n'
        'define i32 @main() {' + guarda + '  ret i32 0\n}\n', encoding='utf-8')
    executar(nome + '-compilar', [clang, trecho, '-O2', '-o', exe])
    executar(nome + '-executar', [exe], sucesso=bool(compatibilidade), limite=30)
    imagens.append(dict(nome=nome, sha256=hash_arquivo(exe), ir_sha256=hash_arquivo(trecho)))

for excecoes in ['checagem', 'tabelas']:
    nome = 'finally-arc-' + excecoes
    exe = saida / (nome + '.exe')
    executar(nome + '-compilar', [cli, 'aot', fonte, exe, '--optimize',
        '--memoria', 'arc', '--excecoes', excecoes])
    estresse = ambiente.copy()
    estresse['DARTFORGE_GC_STRESS'] = '1'
    obtido = executar(nome + '-executar', [exe], env=estresse, limite=120)
    assert obtido == esperado, nome
    imagens.append(dict(nome=nome, sha256=hash_arquivo(exe), saida_igual_dart=True))

assert hash_arquivo(cli) == hash_cli
(saida / 'executaveis.json').write_text(json.dumps(dict(imagens=imagens,
    ir_real_sha256=hash_arquivo(ir), funcao_da_guarda=nome_funcao,
    guarda_sha256=hash_arquivo(saida / 'guarda-extraida.ll')), indent=2), encoding='utf-8')
print('Guarda extraída aceita ABI 1, rejeita ABI 0; dois AOT ARC iguais ao Dart.', flush=True)
