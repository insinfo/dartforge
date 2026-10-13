#!/usr/bin/env python3
"""Breaks a section of the executable down by origin, from the link map.

Usage:
    python tools/size/breakdown.py <map> [--section 0001] [--groups N] [--symbols N]

The map is the `lld-link` one (Windows): compile in production with
`DARTFORGE_LINK_MAP=1` (`dartforge aot x.dart x.exe --optimize`), and it comes
out in `x.map`. Section `0001` is `.text`; `0002`, `.rdata`.

Each symbol takes from its address up to the next one. The origin is:
* `dart:<library>` for the SDK's Dart code (`df.dart$3a<lib>…`), from LTO;
* `dart-prog/…` for the rest of the generated code (the program, `df.*` helpers);
* `rust:<crate>` for the runtime (the `staticlib` member);
* `c/asm:<object>` for C (ring, zlib) and the CRT.

See docs/NATIVO-PODA-DE-TABELAS.md §3.10 and docs/PLANO-TAMANHO-DESEMPENHO.md §1.
"""
import argparse
import collections
import re
import sys

SECTION = re.compile(r'\s*(000[1-9]):([0-9a-f]{8}) ([0-9a-f]{8})H\s+(\S+)')
SYMBOL = re.compile(r'\s*(000[1-9]):([0-9a-f]{8})\s+(\S+)\s+[0-9a-f]{16}\s+(?:f\s+)?(?:i\s+)?(\S+)?')


def read(link_map, section):
    """The section's symbols, by address, and the end of the section."""
    end = 0
    symbols = {}
    for line in open(link_map, encoding='utf-8', errors='replace'):
        m = SECTION.match(line)
        if m:
            if m.group(1) == section:
                end = max(end, int(m.group(2), 16) + int(m.group(3), 16))
            continue
        m = SYMBOL.match(line)
        if m and m.group(1) == section:
            a = int(m.group(2), 16)
            # Among names at the same address, keep the one that is not auxiliary.
            if a not in symbols or symbols[a][0].startswith(('?dtor', '?catch', 'L$', '$')):
                symbols[a] = (m.group(3), m.group(4) or '')
    return symbols, end


def origin(obj, name):
    if 'lto' in obj:
        m = re.match(r'"?df\.dart\$3a([a-z_]+)', name)
        if m:
            return 'dart:' + m.group(1)
        if name.startswith('df.') or name.startswith('dfc.'):
            m = re.match(r'dfc?\.([a-z]+)', name)
            return 'dart-prog/df.' + (m.group(1) if m else '')
        return 'dart-prog/' + name[:25]
    if ':' in obj:
        member = obj.split(':', 1)[1]
        m = re.search(r'\.([a-z0-9_]+)-[0-9a-f]{16}\.', member)
        if m:
            return 'rust:' + m.group(1)
        return 'c/asm:' + member
    return 'other:' + obj


def main():
    # The Windows console is not UTF-8 by default.
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('link_map')
    ap.add_argument('--section', default='0001')
    ap.add_argument('--groups', type=int, default=40, help='how many origins to list')
    ap.add_argument('--symbols', type=int, default=0, help='how many of the largest symbols to list')
    a = ap.parse_args()
    symbols, end = read(a.link_map, a.section)
    if not symbols:
        sys.exit(f'no symbol in section {a.section} of {a.link_map}')
    addresses = sorted(symbols)
    by_origin = collections.Counter()
    largest = []
    for i, e in enumerate(addresses):
        size = (addresses[i + 1] if i + 1 < len(addresses) else end) - e
        name, obj = symbols[e]
        o = origin(obj, name)
        by_origin[o] += size
        largest.append((size, name, o))
    total = sum(by_origin.values())
    print(f'section {a.section}: {end:,} bytes; attributed {total:,}; before the first symbol {addresses[0]:,}')
    groups = collections.Counter()
    for o, v in by_origin.items():
        groups['DART' if o.startswith('dart') else 'RUST' if o.startswith('rust') else 'C/ASM/other'] += v
    for g, v in groups.most_common():
        print(f'  {g:12} {v:>12,}  {100 * v / total:5.1f}%')
    print()
    for o, v in by_origin.most_common(a.groups):
        print(f'{v:>12,}  {o}')
    if a.symbols:
        print()
        for t, n, o in sorted(largest, reverse=True)[:a.symbols]:
            print(f'{t:>10,}  {o:24} {n[:110]}')


if __name__ == '__main__':
    main()
