"""Compares two outputs of `dartforge-parity scoreboard --details` code by code.

Usage: python scripts/compare-scoreboards.py <scoreboard-a> <scoreboard-b>
Prints the codes whose hits, FP or FN changed from A to B, worst first.
"""
import re
import sys


def table(path):
    rows = {}
    inside = False
    for l in open(path, encoding='utf-8'):
        if l.startswith('code '):
            inside = True
            continue
        if not inside:
            continue
        m = re.match(r'^(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)', l)
        if not m:
            if l.strip() == '':
                inside = False
            continue
        code, oracle, ours, hit, msg, pos, fp, fn = m.groups()
        rows[code] = (int(hit), int(fp), int(fn), int(pos))
    return rows


a, b = table(sys.argv[1]), table(sys.argv[2])
changes = []
for code in sorted(set(a) | set(b)):
    va, vb = a.get(code, (0, 0, 0, 0)), b.get(code, (0, 0, 0, 0))
    if va != vb:
        # Worse: fewer hits or more FP.
        weight = (vb[0] - va[0]) - (vb[1] - va[1])
        changes.append((weight, code, va, vb))
changes.sort()
for weight, code, va, vb in changes:
    print(f'{code:55} hit {va[0]:5}->{vb[0]:5}  FP {va[1]:4}->{vb[1]:4}  FN {va[2]:4}->{vb[2]:4}  pos {va[3]:3}->{vb[3]:3}')
