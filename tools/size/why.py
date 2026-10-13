#!/usr/bin/env python3
"""Why did this stay in the executable? Reads the table pruning report.

Usage:
    python tools/size/why.py <report.tsv> <regex> [--all]
        the chain of causes, up to the root, of each live symbol whose name
        matches the regex (the first one, or all with --all);
    python tools/size/why.py <report.tsv> --summary [--map MAP] [-n N]
        the selectors that keep the most table pairs alive and, with the link
        map, how many bytes of `.text` each library and each selector keep
        alive (the closure of what the pair pulled in).

The report comes from the production compilation with
`DARTFORGE_WHY=<report.tsv>` (docs/NATIVO-PODA-DE-TABELAS.md §3.10): one line
per live symbol, with the cause of the first time it got in — `root`, `ref`
(cited by `parent`) or `table` (pair of the table `parent` through
`selector`, which the function `caller` called first). The map is the one of
`DARTFORGE_LINK_MAP=1`.
"""
import argparse
import collections
import re
import sys


def read_report(path):
    causes = {}
    with open(path, encoding='utf-8') as f:
        next(f)
        for line in f:
            c = line.rstrip('\n').split('\t')
            if len(c) >= 5:
                causes[c[0]] = (c[1], c[2], c[3], c[4])
    return causes


def chain(causes, symbol):
    seen = set()
    while symbol in causes and symbol not in seen:
        seen.add(symbol)
        cause, parent, selector, caller = causes[symbol]
        if cause == 'root':
            print(f'  {symbol}  [root]')
            return
        if cause == 'table':
            print(f'  {symbol}  <- pair of the table {parent}, selector {selector} (first called by {caller})')
        else:
            print(f'  {symbol}  <- cited by {parent}')
        symbol = parent


def sizes(link_map):
    """Bytes of each `.text` symbol in the `lld-link` map."""
    rx = re.compile(r'\s*0001:([0-9a-f]{8})\s+(\S+)\s+[0-9a-f]{16}')
    end = 0
    addresses = {}
    for line in open(link_map, encoding='utf-8', errors='replace'):
        m = re.match(r'\s*0001:([0-9a-f]{8}) ([0-9a-f]{8})H', line)
        if m:
            end = max(end, int(m.group(1), 16) + int(m.group(2), 16))
            continue
        m = rx.match(line)
        if m:
            addresses.setdefault(int(m.group(1), 16), m.group(2).strip('"'))
    order = sorted(addresses)
    t = {}
    for i, e in enumerate(order):
        name = addresses[e].split('.llvm.')[0]
        t[name] = t.get(name, 0) + (order[i + 1] if i + 1 < len(order) else end) - e
    return t


def summary(causes, link_map, n):
    by_selector = collections.Counter(s for (c, _, s, _) in causes.values() if c == 'table')
    print(f'{len(causes):,} live symbols; {sum(by_selector.values()):,} got in through a table pair')
    print('\nselectors that pulled in the most entries:')
    for s, k in by_selector.most_common(n):
        print(f'{k:>7,}  {s}')
    if not link_map:
        return
    t = sizes(link_map)
    # The "origin" of each symbol: the first table pair in its chain (the
    # selector that pulled it in) or "direct" if the chain reaches the root
    # without a pair.
    memo = {}

    def origin(s):
        path = []
        while s in causes and s not in memo:
            path.append(s)
            cause, parent, selector, _ = causes[s]
            if cause == 'root':
                memo[s] = 'direct'
                break
            if cause == 'table':
                memo[s] = selector
                break
            s = parent
        r = memo.get(s, 'direct')
        for x in path:
            memo[x] = r
        return r

    by_origin = collections.Counter()
    by_library = collections.Counter()
    for s, b in t.items():
        if s in causes:
            by_origin[origin(s)] += b
        m = re.match(r'df\.dart\$3a([a-z_]+)', s)
        by_library['dart:' + m.group(1) if m else 'other'] += b
    print('\n.text bytes per library:')
    for k, v in by_library.most_common(n):
        print(f'{v:>12,}  {k}')
    print('\n.text bytes by the selector that pulled them in (the first table pair in the chain):')
    for k, v in by_origin.most_common(n):
        print(f'{v:>12,}  {k}')


def main():
    # The Windows console is not UTF-8 by default.
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('report')
    ap.add_argument('regex', nargs='?')
    ap.add_argument('--all', action='store_true')
    ap.add_argument('--summary', action='store_true')
    ap.add_argument('--map', dest='link_map')
    ap.add_argument('-n', type=int, default=30)
    a = ap.parse_args()
    causes = read_report(a.report)
    if a.summary:
        summary(causes, a.link_map, a.n)
        return
    if not a.regex:
        sys.exit('give the symbol regex, or --summary')
    rx = re.compile(a.regex)
    found = sorted(s for s in causes if rx.search(s))
    if not found:
        sys.exit(f'no live symbol matches {a.regex!r}')
    for s in found if a.all else found[:1]:
        print(s)
        chain(causes, s)
    if not a.all and len(found) > 1:
        print(f'({len(found) - 1} more symbols match; --all lists them all)')


if __name__ == '__main__':
    main()
