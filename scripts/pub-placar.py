#!/usr/bin/env python3
"""Placar do backend nativo sobre pacotes reais do pub
(docs/NATIVO-PROJETOS-REAIS.md §0).

O projeto `corpus/pub` depende de pacotes populares com as versões fixas.
Para cada pacote resolvido, os programas são todo `example/*.dart` que tem
`main` e os N menores `test/**/*_test.dart` (padrão 3). Cada programa roda
na VM (`dart --enable-asserts --packages=…`, o oráculo do corpus) e no
nativo (`dartforge aot`, perfil de desenvolvimento), com o diretório (uma
cópia) do pacote como corrente; a saída padrão (sem os tempos `00:00` do
`package:test` e as sementes `Random Seed: N`) e o código de saída são
comparados. Programa que a VM não compila (código 254) fica fora do placar.

uso:
  python3 scripts/pub-placar.py --dartforge <dartforge> [--dart dart]
      [--projeto corpus/pub] [--trabalho DIR] [--shard i/n]
      [--limite-testes 3] [--tempo-compilacao 900] [--tempo-execucao 120]
      [--filtro TEXTO]

Grava em <trabalho>: `relatorio.txt` (uma linha por programa, a linha
`PLACAR:` e as falhas agrupadas pela primeira linha do erro) e
`resultados.json`. Sai com 0 mesmo com falhas (o placar é o produto).
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from urllib.parse import unquote, urlparse

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(AQUI)


def caminho_de_uri(uri, base):
    if uri.startswith('file:'):
        p = unquote(urlparse(uri).path)
        # file:///C:/x no Windows
        if re.match(r'^/[A-Za-z]:', p):
            p = p[1:]
        return os.path.normpath(p)
    return os.path.normpath(os.path.join(base, unquote(uri)))


def diretos(pubspec):
    saida = []
    dentro = False
    for linha in open(pubspec, encoding='utf8'):
        if re.match(r'^dependencies:\s*$', linha):
            dentro = True
            continue
        if dentro and re.match(r'^\S', linha):
            dentro = False
        m = re.match(r'^  ([a-z0-9_]+):', linha)
        if dentro and m:
            saida.append(m.group(1))
    return saida


def programas(projeto, trabalho, limite_testes):
    cfg_arq = os.path.join(projeto, '.dart_tool', 'package_config.json')
    cfg = json.load(open(cfg_arq, encoding='utf8'))
    base = os.path.dirname(cfg_arq)
    nomes = set(diretos(os.path.join(projeto, 'pubspec.yaml')))
    pkgs = os.path.join(trabalho, 'pkgs')
    saida = []
    for p in sorted(cfg['packages'], key=lambda p: p['name']):
        if p['name'] not in nomes:
            continue
        origem = caminho_de_uri(p['rootUri'], base)
        copia = os.path.join(pkgs, p['name'])
        if not os.path.isdir(copia):
            shutil.copytree(origem, copia, ignore=shutil.ignore_patterns('.dart_tool', 'build'))
        ex = os.path.join(copia, 'example')
        if os.path.isdir(ex):
            for f in sorted(os.listdir(ex)):
                c = os.path.join(ex, f)
                if f.endswith('.dart') and re.search(r'\bmain\s*\(', open(c, encoding='utf8', errors='replace').read()):
                    saida.append((p['name'], c))
        testes = []
        for d, _, fs in os.walk(os.path.join(copia, 'test')):
            for f in fs:
                if f.endswith('_test.dart'):
                    testes.append(os.path.join(d, f))
        testes.sort(key=lambda c: (os.path.getsize(c), c.replace(os.sep, '/')))
        saida.extend((p['name'], c) for c in testes[:limite_testes])
    return saida


def chave(trabalho, nome, arq):
    return nome + '/' + os.path.relpath(arq, os.path.join(trabalho, 'pkgs', nome)).replace(os.sep, '/')


def limitar_memoria(mb):
    """No Linux, o `preexec_fn` do processo: sessão própria (o limite de
    tempo mata o grupo inteiro — o programa que abre processos filhos, o
    Clang e o ligador do dartforge) e `RLIMIT_DATA` de `mb` MiB (o heap e o
    `mmap` gravável; a reserva de endereços da VM não conta). O programa que
    estoura falha sozinho, com estado `memoria`, em vez de o OOM do sistema
    derrubar o runner."""
    if os.name == 'nt':
        return None
    import resource

    def aplicar():
        os.setsid()
        if mb:
            b = mb * 1024 * 1024
            resource.setrlimit(resource.RLIMIT_DATA, (b, b))
    return aplicar


def rodar(cmd, cwd, limite, env, memoria_mb=0):
    t = time.time()
    p = subprocess.Popen(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env,
                         preexec_fn=limitar_memoria(memoria_mb))
    try:
        out, err = p.communicate(timeout=limite)
        return p.returncode, out.decode('utf8', 'replace'), err.decode('utf8', 'replace'), time.time() - t
    except subprocess.TimeoutExpired:
        if os.name == 'nt':
            subprocess.run(['taskkill', '/F', '/T', '/PID', str(p.pid)], capture_output=True)
        else:
            import signal
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except OSError:
                pass
        out, err = p.communicate()
        return 'timeout', out.decode('utf8', 'replace'), err.decode('utf8', 'replace'), time.time() - t


def sem_memoria(rc, texto):
    return rc in (-9, 137) or re.search(
        r'out of memory|Out of memory|memory allocation of|Cannot allocate memory|bad_alloc|Exhausted heap space', texto)


def normalizar(s):
    s = s.replace('\r\n', '\n')
    s = re.sub(r'^\d\d:\d\d ', '', s, flags=re.M)
    s = re.sub(r'Random Seed: \d+', 'Random Seed: N', s)
    # instantes impressos pelos programas (`logging`: `2026-10-01 02:03:22.979720`)
    s = re.sub(r'\d{4}-\d\d-\d\d[ T]\d\d:\d\d:\d\d(\.\d+)?', '<instante>', s)
    return s


def um(a, cfg, k, nome, arq, env):
    cwd = os.path.join(a.trabalho, 'pkgs', nome)
    res = {'k': k}
    rc, vout, verr, _ = rodar([a.dart, '--enable-asserts', '--packages=' + cfg, arq], cwd, a.tempo_execucao, env,
                              a.memoria_execucao)
    res['vm_rc'] = rc
    if rc == 'timeout' or rc == 254 or (rc != 0 and sem_memoria(rc, verr)):
        res['estado'] = 'vm-invalido'
        res['erro'] = ((verr.strip().splitlines() or ['timeout na VM'])[0])[:300]
        return res
    sufixo = '.exe' if os.name == 'nt' else ''
    exe = os.path.join(a.trabalho, 'bin', hashlib.sha1(k.encode()).hexdigest()[:16] + sufixo)
    os.makedirs(os.path.dirname(exe), exist_ok=True)
    env_df = dict(env)
    # Os pacotes publicados já trazem o código gerado; o motor de build
    # tomaria a cópia do pacote como raiz e cobraria as dependências de
    # desenvolvimento dela.
    env_df['DARTFORGE_BUILD_COMPILANDO_EXECUTOR'] = '1'
    rc, out, err, dt = rodar([a.dartforge, 'aot', arq, exe, '--packages', cfg], cwd, a.tempo_compilacao, env_df,
                             a.memoria_compilacao)
    res['t_compilacao'] = round(dt, 1)
    if rc != 0 and sem_memoria(rc, err + out):
        res['estado'] = 'memoria'
        res['erro'] = 'memória esgotada na compilação (limite %d MiB)' % a.memoria_compilacao
        res['saida_erro'] = (err + out)[-4000:]
        return res
    if rc != 0:
        linhas = [l for l in (err + out).strip().splitlines() if l.strip()]
        res['estado'] = 'compilacao'
        res['erro'] = (linhas[0] if linhas else 'timeout na compilação' if rc == 'timeout' else str(rc))[:300]
        res['saida_erro'] = (err + out)[-4000:]
        return res
    # O equivalente do `--packages` que o oráculo recebe: `Isolate.packageConfig`
    # e `Isolate.resolvePackageUriSync` do executável leem este arquivo.
    env_exe = dict(env)
    env_exe['DARTFORGE_PACKAGE_CONFIG'] = cfg
    rc, out, err, dt = rodar([exe], cwd, a.tempo_execucao, env_exe, a.memoria_execucao)
    res['t_execucao'] = round(dt, 1)
    if rc != 0 and rc != res['vm_rc'] and sem_memoria(rc, err):
        res['estado'] = 'memoria'
        res['erro'] = 'memória esgotada na execução (limite %d MiB)' % a.memoria_execucao
        res['stderr'] = err[-3000:]
        return res
    try:
        os.remove(exe)
    except OSError:
        pass
    if rc == 'timeout':
        res['estado'] = 'timeout'
        res['erro'] = 'timeout na execução'
    elif normalizar(out) == normalizar(vout) and (rc == res['vm_rc'] or (rc != 0 and res['vm_rc'] != 0)):
        res['estado'] = 'ok'
        return res
    else:
        quebra = isinstance(rc, int) and (rc < 0 or rc > 255)
        res['estado'] = 'crash' if quebra else 'divergente'
        linhas = [l for l in err.strip().splitlines() if l.strip()]
        res['erro'] = (linhas[0] if linhas else 'saída diferente (código %s, VM %s)' % (rc, res['vm_rc']))[:300]
        if quebra:
            res['erro'] = 'código de saída %#x: %s' % (rc & 0xFFFFFFFF, res['erro'])
    res['rc'] = rc
    res['stderr'] = err[-3000:]
    va, na = normalizar(vout).splitlines(), normalizar(out).splitlines()
    i = 0
    while i < min(len(va), len(na)) and va[i] == na[i]:
        i += 1
    res['diferenca'] = {'linha': i + 1, 'vm': va[i:i + 3], 'nativo': na[i:i + 3]}
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--dartforge', required=True)
    ap.add_argument('--dart', default='dart')
    ap.add_argument('--projeto', default=os.path.join(RAIZ, 'corpus', 'pub'))
    ap.add_argument('--trabalho', default=os.path.join(RAIZ, 'target', 'pub-placar'))
    ap.add_argument('--shard', default='1/1')
    ap.add_argument('--limite-testes', type=int, default=3)
    ap.add_argument('--tempo-compilacao', type=int, default=900)
    ap.add_argument('--tempo-execucao', type=int, default=120)
    ap.add_argument('--memoria-compilacao', type=int, default=6000,
                    help='MiB de dados (RLIMIT_DATA) do dartforge e dos filhos (Linux; 0 = sem limite)')
    ap.add_argument('--memoria-execucao', type=int, default=3000,
                    help='MiB de dados da VM do oráculo e do executável (Linux; 0 = sem limite)')
    ap.add_argument('--filtro', default='')
    a = ap.parse_args()
    a.trabalho = os.path.abspath(a.trabalho)
    a.dartforge = os.path.abspath(a.dartforge) if os.path.exists(a.dartforge) else a.dartforge
    os.makedirs(a.trabalho, exist_ok=True)
    projeto = os.path.abspath(a.projeto)
    cfg = os.path.join(projeto, '.dart_tool', 'package_config.json')
    if not os.path.exists(cfg):
        subprocess.run([a.dart, 'pub', 'get'], cwd=projeto, check=True)
    i, n = (int(x) for x in a.shard.split('/'))
    itens = [(chave(a.trabalho, nome, arq), nome, arq) for nome, arq in programas(projeto, a.trabalho, a.limite_testes)]
    itens.sort()
    itens = [x for x in itens if a.filtro in x[0]][i - 1::n]
    env = dict(os.environ)
    if os.name == 'nt':
        env.pop('HOME', None)
    print('%d programas (shard %s)' % (len(itens), a.shard), flush=True)
    resultados = []
    relatorio = []
    for k, nome, arq in itens:
        res = um(a, cfg, k, nome, arq, env)
        resultados.append(res)
        linha = '%-11s %s  %s' % (res['estado'], k, res.get('erro', ''))
        relatorio.append(linha)
        print(linha, flush=True)
    cont = {}
    grupos = {}
    for r in resultados:
        cont[r['estado']] = cont.get(r['estado'], 0) + 1
        if r['estado'] not in ('ok', 'vm-invalido'):
            g = re.sub(r'\(\S+:\d+:\d+\)', '', r.get('erro', ''))
            g = re.sub(r'\d+', 'N', g)
            grupos.setdefault((r['estado'], g), []).append(r['k'])
    validos = sum(v for e, v in cont.items() if e != 'vm-invalido')
    placar = 'PLACAR: %d/%d ok (%s)' % (cont.get('ok', 0), validos, ', '.join('%s %d' % kv for kv in sorted(cont.items())))
    relatorio.append('')
    relatorio.append(placar)
    relatorio.append('')
    relatorio.append('falhas por causa (primeira linha do erro):')
    for (e, g), ks in sorted(grupos.items(), key=lambda x: (-len(x[1]), x[0])):
        relatorio.append('%4d %-11s %s' % (len(ks), e, g[:200]))
        for k in ks:
            relatorio.append('       ' + k)
    open(os.path.join(a.trabalho, 'relatorio.txt'), 'w', encoding='utf8').write('\n'.join(relatorio) + '\n')
    json.dump(resultados, open(os.path.join(a.trabalho, 'resultados.json'), 'w', encoding='utf8'), indent=1, ensure_ascii=False)
    print('\n' + placar)


if __name__ == '__main__':
    sys.exit(main())
