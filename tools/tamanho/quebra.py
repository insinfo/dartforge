#!/usr/bin/env python3
"""Quebra uma seção do executável por origem, a partir do mapa da ligação.

Uso:
    python tools/tamanho/quebra.py <mapa> [--secao 0001] [--grupos N] [--simbolos N]

O mapa é o do `lld-link` (Windows): compile em produção com
`DARTFORGE_MAPA_DA_LIGACAO=1` (`dartforge aot x.dart x.exe --optimize`), e ele
sai em `x.map`. A seção `0001` é o `.text`; `0002`, o `.rdata`.

Cada símbolo ocupa do endereço dele até o próximo. A origem é:
* `dart:<biblioteca>` para o código Dart do SDK (`df.dart$3a<lib>…`), vindo da LTO;
* `dart-prog/…` para o resto do código gerado (o programa, ajudantes `df.*`);
* `rust:<crate>` para o runtime (o membro da `staticlib`);
* `c/asm:<objeto>` para o C (ring, zlib) e a CRT.

Ver docs/NATIVO-PODA-DE-TABELAS.md §3.10 e docs/PLANO-TAMANHO-DESEMPENHO.md §1.
"""
import argparse
import collections
import re
import sys

SECAO = re.compile(r'\s*(000[1-9]):([0-9a-f]{8}) ([0-9a-f]{8})H\s+(\S+)')
SIMBOLO = re.compile(r'\s*(000[1-9]):([0-9a-f]{8})\s+(\S+)\s+[0-9a-f]{16}\s+(?:f\s+)?(?:i\s+)?(\S+)?')


def ler(mapa, secao):
    """Os símbolos da seção, por endereço, e o fim da seção."""
    fim = 0
    simbolos = {}
    for linha in open(mapa, encoding='utf-8', errors='replace'):
        m = SECAO.match(linha)
        if m:
            if m.group(1) == secao:
                fim = max(fim, int(m.group(2), 16) + int(m.group(3), 16))
            continue
        m = SIMBOLO.match(linha)
        if m and m.group(1) == secao:
            a = int(m.group(2), 16)
            # Entre nomes no mesmo endereço, fica o que não é auxiliar.
            if a not in simbolos or simbolos[a][0].startswith(('?dtor', '?catch', 'L$', '$')):
                simbolos[a] = (m.group(3), m.group(4) or '')
    return simbolos, fim


def origem(objeto, nome):
    if 'lto' in objeto:
        m = re.match(r'"?df\.dart\$3a([a-z_]+)', nome)
        if m:
            return 'dart:' + m.group(1)
        if nome.startswith('df.') or nome.startswith('dfc.'):
            m = re.match(r'dfc?\.([a-z]+)', nome)
            return 'dart-prog/df.' + (m.group(1) if m else '')
        return 'dart-prog/' + nome[:25]
    if ':' in objeto:
        membro = objeto.split(':', 1)[1]
        m = re.search(r'\.([a-z0-9_]+)-[0-9a-f]{16}\.', membro)
        if m:
            return 'rust:' + m.group(1)
        return 'c/asm:' + membro
    return 'outro:' + objeto


def main():
    # A saída tem acentos; o console do Windows não é UTF-8 por padrão.
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('mapa')
    ap.add_argument('--secao', default='0001')
    ap.add_argument('--grupos', type=int, default=40, help='quantas origens listar')
    ap.add_argument('--simbolos', type=int, default=0, help='quantos maiores símbolos listar')
    a = ap.parse_args()
    simbolos, fim = ler(a.mapa, a.secao)
    if not simbolos:
        sys.exit(f'nenhum símbolo na seção {a.secao} de {a.mapa}')
    enderecos = sorted(simbolos)
    por_origem = collections.Counter()
    maiores = []
    for i, e in enumerate(enderecos):
        tamanho = (enderecos[i + 1] if i + 1 < len(enderecos) else fim) - e
        nome, objeto = simbolos[e]
        o = origem(objeto, nome)
        por_origem[o] += tamanho
        maiores.append((tamanho, nome, o))
    total = sum(por_origem.values())
    print(f'seção {a.secao}: {fim:,} bytes; atribuídos {total:,}; antes do primeiro símbolo {enderecos[0]:,}')
    grupos = collections.Counter()
    for o, v in por_origem.items():
        grupos['DART' if o.startswith('dart') else 'RUST' if o.startswith('rust') else 'C/ASM/outro'] += v
    for g, v in grupos.most_common():
        print(f'  {g:12} {v:>12,}  {100 * v / total:5.1f}%')
    print()
    for o, v in por_origem.most_common(a.grupos):
        print(f'{v:>12,}  {o}')
    if a.simbolos:
        print()
        for t, n, o in sorted(maiores, reverse=True)[:a.simbolos]:
            print(f'{t:>10,}  {o:24} {n[:110]}')


if __name__ == '__main__':
    main()
