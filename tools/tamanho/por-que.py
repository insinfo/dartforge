#!/usr/bin/env python3
"""Por que isto ficou no executável? Lê o relatório da poda das tabelas.

Uso:
    python tools/tamanho/por-que.py <relatorio.tsv> <regex> [--todos]
        a cadeia de causas, até a raiz, de cada símbolo vivo cujo nome casa
        com a regex (o primeiro, ou todos com --todos);
    python tools/tamanho/por-que.py <relatorio.tsv> --resumo [--mapa MAPA] [-n N]
        os seletores que mais mantêm pares de tabela vivos e, com o mapa da
        ligação, quantos bytes do `.text` cada biblioteca e cada seletor
        mantêm vivos (o fecho do que o par puxou).

O relatório sai da compilação de produção com
`DARTFORGE_POR_QUE=<relatorio.tsv>` (docs/NATIVO-PODA-DE-TABELAS.md §3.10):
uma linha por símbolo vivo, com a causa da primeira vez em que ele entrou —
`raiz`, `ref` (citado por `pai`) ou `tabela` (par da tabela `pai` pelo
`seletor`, que a função `chamador` chamou primeiro). O mapa é o de
`DARTFORGE_MAPA_DA_LIGACAO=1`.
"""
import argparse
import collections
import re
import sys


def ler_relatorio(caminho):
    causas = {}
    with open(caminho, encoding='utf-8') as f:
        next(f)
        for linha in f:
            c = linha.rstrip('\n').split('\t')
            if len(c) >= 5:
                causas[c[0]] = (c[1], c[2], c[3], c[4])
    return causas


def cadeia(causas, simbolo):
    vistos = set()
    while simbolo in causas and simbolo not in vistos:
        vistos.add(simbolo)
        causa, pai, seletor, chamador = causas[simbolo]
        if causa == 'raiz':
            print(f'  {simbolo}  [raiz]')
            return
        if causa == 'tabela':
            print(f'  {simbolo}  <- par da tabela {pai}, seletor {seletor} (chamado primeiro por {chamador})')
        else:
            print(f'  {simbolo}  <- citado por {pai}')
        simbolo = pai


def tamanhos(mapa):
    """Bytes de cada símbolo do `.text` no mapa do `lld-link`."""
    rx = re.compile(r'\s*0001:([0-9a-f]{8})\s+(\S+)\s+[0-9a-f]{16}')
    fim = 0
    enderecos = {}
    for linha in open(mapa, encoding='utf-8', errors='replace'):
        m = re.match(r'\s*0001:([0-9a-f]{8}) ([0-9a-f]{8})H', linha)
        if m:
            fim = max(fim, int(m.group(1), 16) + int(m.group(2), 16))
            continue
        m = rx.match(linha)
        if m:
            enderecos.setdefault(int(m.group(1), 16), m.group(2).strip('"'))
    ordem = sorted(enderecos)
    t = {}
    for i, e in enumerate(ordem):
        nome = enderecos[e].split('.llvm.')[0]
        t[nome] = t.get(nome, 0) + (ordem[i + 1] if i + 1 < len(ordem) else fim) - e
    return t


def resumo(causas, mapa, n):
    por_seletor = collections.Counter(s for (c, _, s, _) in causas.values() if c == 'tabela')
    print(f'{len(causas):,} símbolos vivos; {sum(por_seletor.values()):,} entraram por um par de tabela')
    print('\nseletores que mais puxaram entradas:')
    for s, k in por_seletor.most_common(n):
        print(f'{k:>7,}  {s}')
    if not mapa:
        return
    t = tamanhos(mapa)
    # A "origem" de cada símbolo: o primeiro par de tabela na cadeia dele
    # (o seletor que o puxou) ou "direto" se a cadeia chega à raiz sem par.
    memo = {}

    def origem(s):
        caminho = []
        while s in causas and s not in memo:
            caminho.append(s)
            causa, pai, seletor, _ = causas[s]
            if causa == 'raiz':
                memo[s] = 'direto'
                break
            if causa == 'tabela':
                memo[s] = seletor
                break
            s = pai
        r = memo.get(s, 'direto')
        for x in caminho:
            memo[x] = r
        return r

    por_origem = collections.Counter()
    por_biblioteca = collections.Counter()
    for s, b in t.items():
        if s in causas:
            por_origem[origem(s)] += b
        m = re.match(r'df\.dart\$3a([a-z_]+)', s)
        por_biblioteca['dart:' + m.group(1) if m else 'outro'] += b
    print('\nbytes do .text por biblioteca:')
    for k, v in por_biblioteca.most_common(n):
        print(f'{v:>12,}  {k}')
    print('\nbytes do .text pelo seletor que puxou (o primeiro par de tabela na cadeia):')
    for k, v in por_origem.most_common(n):
        print(f'{v:>12,}  {k}')


def main():
    # A saída tem acentos; o console do Windows não é UTF-8 por padrão.
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('relatorio')
    ap.add_argument('regex', nargs='?')
    ap.add_argument('--todos', action='store_true')
    ap.add_argument('--resumo', action='store_true')
    ap.add_argument('--mapa')
    ap.add_argument('-n', type=int, default=30)
    a = ap.parse_args()
    causas = ler_relatorio(a.relatorio)
    if a.resumo:
        resumo(causas, a.mapa, a.n)
        return
    if not a.regex:
        sys.exit('diga a regex do símbolo, ou --resumo')
    rx = re.compile(a.regex)
    achados = sorted(s for s in causas if rx.search(s))
    if not achados:
        sys.exit(f'nenhum símbolo vivo casa com {a.regex!r}')
    for s in achados if a.todos else achados[:1]:
        print(s)
        cadeia(causas, s)
    if not a.todos and len(achados) > 1:
        print(f'(mais {len(achados) - 1} símbolos casam; --todos lista todos)')


if __name__ == '__main__':
    main()
