"""Attribute linked text from a Mach-O link map; symbol ranges are not timings."""

import argparse
import hashlib
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
parser.add_argument('map', type=Path)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
lines = args.map.read_text().splitlines()
sections, symbols, objects = [], [], {}
for line in lines:
    section = re.fullmatch(r'(0x[0-9a-fA-F]+)\s+(0x[0-9a-fA-F]+)\s+(__\w+)\s+(__\w+)', line)
    if section:
        sections.append({'address': int(section[1], 16), 'bytes': int(section[2], 16),
                         'segment': section[3], 'section': section[4]})
    obj = re.fullmatch(r'\[\s*(\d+)\]\s+(.+)', line)
    if obj:
        objects[int(obj[1])] = obj[2]
    symbol = re.fullmatch(r'(0x[0-9a-fA-F]+)\s+(0x[0-9a-fA-F]+)\s+\[\s*(\d+)\]\s+(.+)', line)
    if symbol:
        symbols.append({'address': int(symbol[1], 16), 'bytes': int(symbol[2], 16),
                        'object': int(symbol[3]), 'name': symbol[4]})
text = next(row for row in sections if row['segment'] == '__TEXT' and row['section'] == '__text')
text_end = text['address'] + text['bytes']
selected = [row for row in symbols if text['address'] <= row['address'] < text_end
            and row['address'] + row['bytes'] <= text_end]
for row in selected:
    row['object_path'] = objects.get(row['object'])
intervals = sorted(set((row['address'], row['address'] + row['bytes']) for row in selected))
covered, end = 0, text['address']
for start, stop in intervals:
    covered += max(0, stop - max(start, end))
    end = max(end, stop)
summary = {
    'binary': str(args.binary.resolve()),
    'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
    'binary_bytes': args.binary.stat().st_size,
    'link_map_sha256': hashlib.sha256(args.map.read_bytes()).hexdigest(),
    'sections': sections,
    'text_symbol_entries': len(selected),
    'text_bytes_covered_by_symbol_range_union': covered,
    'largest_text_symbols': sorted(selected, key=lambda row: row['bytes'], reverse=True)[:50],
    'scope': 'Linked symbol size only. Aliases and overlapping symbol ranges may exist; union avoids summing them twice. No performance or future size reduction claim.',
}
args.output.write_text(json.dumps(summary, indent=2) + '\n')
