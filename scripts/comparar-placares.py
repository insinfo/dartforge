"""Compara duas saídas de `dartforge-paridade placar --detalhes` código a código.

Uso: python scripts/comparar-placares.py <placar-a> <placar-b>
Imprime os códigos cujo acerto, FP ou FN mudou de A para B, piores primeiro.
"""
import re
import sys


def tabela(caminho):
    linhas = {}
    dentro = False
    for l in open(caminho, encoding='utf-8'):
        if l.startswith('código '):
            dentro = True
            continue
        if not dentro:
            continue
        m = re.match(r'^(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)', l)
        if not m:
            if l.strip() == '':
                dentro = False
            continue
        cod, oraculo, nosso, acerto, msg, pos, fp, fn = m.groups()
        linhas[cod] = (int(acerto), int(fp), int(fn), int(pos))
    return linhas


a, b = tabela(sys.argv[1]), tabela(sys.argv[2])
mudancas = []
for cod in sorted(set(a) | set(b)):
    va, vb = a.get(cod, (0, 0, 0, 0)), b.get(cod, (0, 0, 0, 0))
    if va != vb:
        # Piora: menos acertos ou mais FP.
        peso = (vb[0] - va[0]) - (vb[1] - va[1])
        mudancas.append((peso, cod, va, vb))
mudancas.sort()
for peso, cod, va, vb in mudancas:
    print(f'{cod:55} acerto {va[0]:5}->{vb[0]:5}  FP {va[1]:4}->{vb[1]:4}  FN {va[2]:4}->{vb[2]:4}  pos {va[3]:3}->{vb[3]:3}')
