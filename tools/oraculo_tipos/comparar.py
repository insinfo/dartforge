"""Compara o despejo de tipos do DartForge com o do oráculo (package:analyzer).

    python tools/oraculo_tipos/comparar.py <nosso.tsv> <oraculo.tsv> [--exemplos N] [--grupo PADRAO]

Cada divergência é classificada pela CAUSA: uma expressão cujo tipo difere e
cujas subexpressões batem com o oráculo (a divergência começa nela; as que a
contêm são cascata). As causas são agrupadas pelo nó e pela forma da
diferença; a saída lista os grupos do maior para o menor, com exemplos.

Os spans do nosso despejo são bytes UTF-8; os do analyzer, unidades UTF-16.
A conversão é feita aqui, lendo o arquivo-fonte.
"""
import sys
import re
from collections import defaultdict

_mapas = {}


def mapa_utf16(arq):
    """byte (UTF-8) -> unidade UTF-16, em cada fronteira de caractere."""
    if arq in _mapas:
        return _mapas[arq]
    try:
        b = open(arq, 'rb').read()
    except OSError:
        _mapas[arq] = None
        return None
    m = {}
    u = 0
    i = 0
    for ch in b.decode('utf-8', errors='replace'):
        m[i] = u
        i += len(ch.encode('utf-8', errors='replace'))
        u += 2 if ord(ch) > 0xFFFF else 1
    m[i] = u
    _mapas[arq] = m
    return m


def ler(caminho, bytes_para_utf16=False):
    d = defaultdict(list)
    with open(caminho, encoding='utf-8', errors='replace') as f:
        for linha in f:
            if linha.startswith('#'):
                continue
            p = linha.rstrip('\n').split('\t')
            if len(p) < 6:
                continue
            ini, comp = int(p[1]), int(p[2])
            if bytes_para_utf16:
                m = mapa_utf16(p[0])
                if m is not None and ini in m and (ini + comp) in m:
                    a, b = m[ini], m[ini + comp]
                    ini, comp = a, b - a
            chave = (p[0].lower(), ini, comp)
            d[chave].append((p[3], p[4], p[5]))
    return d


def normalizar(t):
    return t.replace('*', '')


def forma(nosso, oraculo):
    if nosso == '?':
        return 'não visitada'
    if nosso == 'dynamic':
        return 'nós dynamic'
    if oraculo == 'dynamic':
        return 'oráculo dynamic'
    if nosso.rstrip('?') == oraculo.rstrip('?'):
        return 'nulabilidade'
    base = lambda s: re.sub(r'<.*', '', s).rstrip('?')
    if base(nosso) == base(oraculo):
        return 'argumentos de tipo'
    if 'Function' in oraculo or 'Function' in nosso:
        return 'tipo de função'
    return 'outro tipo'


def main():
    args = sys.argv[1:]
    nexemplos = 3
    filtro = None
    if '--exemplos' in args:
        i = args.index('--exemplos'); nexemplos = int(args[i + 1]); del args[i:i + 2]
    if '--grupo' in args:
        i = args.index('--grupo'); filtro = args[i + 1]; del args[i:i + 2]
    nosso = ler(args[0], bytes_para_utf16=True)
    oraculo = ler(args[1])

    comparadas = iguais = 0
    div = defaultdict(list)
    for chave, nossos in nosso.items():
        if chave not in oraculo:
            continue
        orcs = [o for o in oraculo[chave] if o[1] != '-']
        if not orcs:
            continue
        for (no, tipo, _res) in nossos:
            comparadas += 1
            tipos_o = [normalizar(o[1]) for o in orcs]
            if tipo in tipos_o:
                iguais += 1
                continue
            o = orcs[0]
            div[chave[0]].append((chave[1], chave[1] + chave[2], no, tipo, o[0], normalizar(o[1])))

    total_div = sum(len(v) for v in div.values())
    causas = []
    for arq, lst in div.items():
        lst.sort(key=lambda x: (x[0], -x[1]))
        for i, (ini, fim, no, t, ono, ot) in enumerate(lst):
            contida = False
            for (i2, f2, *_r) in lst[i + 1:]:
                if i2 >= fim:
                    break
                if f2 <= fim and (i2, f2) != (ini, fim):
                    contida = True
                    break
            if not contida:
                causas.append((arq, ini, fim, no, t, ono, ot))

    print(f'expressões comparadas: {comparadas}; iguais: {iguais}; divergentes: {total_div}; causas: {len(causas)}')
    grupos = defaultdict(list)
    for c in causas:
        grupos[f'{c[3]} [{c[5]}] {forma(c[4], c[6])}'].append(c)
    fontes = {}

    def trecho(arq, ini, fim):
        if arq not in fontes:
            try:
                fontes[arq] = open(arq, encoding='utf-8', errors='replace', newline='').read().encode('utf-16-le')
            except OSError:
                fontes[arq] = b''
        u = fontes[arq]
        linha = u[:2 * ini].decode('utf-16-le', errors='replace').count('\n') + 1
        txt = u[2 * ini:2 * min(fim, ini + 90)].decode('utf-16-le', errors='replace').replace('\n', ' ')
        return linha, txt

    for g, lst in sorted(grupos.items(), key=lambda kv: -len(kv[1])):
        if filtro and filtro not in g:
            continue
        print(f'{len(lst):7d}  {g}')
        for (arq, ini, fim, no, t, ono, ot) in lst[:nexemplos]:
            linha, txt = trecho(arq, ini, fim)
            print(f'           {arq.split("/")[-1]}:{linha}  `{txt}`  nós={t}  oráculo={ot}')


if __name__ == '__main__':
    main()
