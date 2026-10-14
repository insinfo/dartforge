#!/usr/bin/env python3
"""Record Mach-O file bytes and sections without modifying measured executables."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess


def inspect(path):
    path = path.resolve(strict=True)
    before = hashlib.sha256(path.read_bytes()).hexdigest()
    result = subprocess.run(['/usr/bin/size', '-m', str(path)],
                            capture_output=True, text=True, check=True)
    segments = {}
    sections = {}
    segment = None
    for line in result.stdout.splitlines():
        match = re.match(r'Segment (\S+): (\d+)', line)
        if match:
            segment = match[1]
            segments[segment] = int(match[2])
        match = re.match(r'\s*Section (\S+): (\d+)', line)
        if match:
            sections[f'{segment}/{match[1]}'] = {
                'bytes': int(match[2]), 'zero_fill': 'zerofill' in line}
    if not segments:
        raise RuntimeError(f'No Mach-O segments found in {path}')
    if hashlib.sha256(path.read_bytes()).hexdigest() != before:
        raise RuntimeError(f'Executable changed during inspection: {path}')
    stat = path.stat()
    return {'path': str(path), 'sha256': before, 'file_bytes': stat.st_size,
            'allocated_disk_bytes': stat.st_blocks * 512,
            'segments_virtual_bytes': segments, 'sections': sections,
            'size_stdout': result.stdout, 'size_stderr': result.stderr}


def inspect_map(path):
    path = path.resolve(strict=True)
    content = path.read_bytes()
    bounds = None
    active = False
    groups = Counter()
    symbols = []
    for line in content.decode('utf-8').splitlines():
        section = re.match(r'0x([0-9a-fA-F]+)\s+0x([0-9a-fA-F]+)\s+__TEXT\s+__text$', line)
        if section:
            start, size = (int(value, 16) for value in section.groups())
            bounds = (start, start + size)
        if line.startswith('# Symbols:'):
            active = True
        elif line.startswith('# Dead Stripped Symbols:'):
            active = False
        if not active or bounds is None:
            continue
        match = re.match(r'0x([0-9a-fA-F]+)\s+0x([0-9a-fA-F]+)\s+\[\s*\d+\]\s+(.+)', line)
        if not match:
            continue
        address, size = (int(value, 16) for value in match.groups()[:2])
        if not bounds[0] <= address < bounds[1] or not size:
            continue
        name = match[3]
        decoded = re.sub(r'\$([0-9a-f]{2})', lambda m: chr(int(m[1], 16)), name)
        library = re.match(r'_df\.(dart:[^.]+|package:[^.]+\.dart)', decoded)
        group = library[1] if library else ('runtime' if name.startswith('_dartforge') else 'other')
        groups[group] += size
        symbols.append({'bytes': size, 'name': decoded, 'group': group})
    if bounds is None:
        raise RuntimeError(f'No __TEXT/__text section in {path}')
    return {'path': str(path), 'sha256': hashlib.sha256(content).hexdigest(),
            'text_section_bytes': bounds[1] - bounds[0],
            'symbol_bytes_by_group': dict(groups.most_common()),
            'largest_symbols': sorted(symbols, key=lambda s: s['bytes'], reverse=True)[:30],
            'limitation': 'Link-map symbol sizes can overlap; group totals are attribution, not file bytes.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', nargs=2, metavar=('LABEL', 'PATH'),
                        action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--link-map', nargs=2, metavar=('LABEL', 'PATH'), action='append', default=[])
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Preserve evidence: output must not already exist')
    binaries = {}
    for label, path in args.binary:
        if label in binaries:
            parser.error(f'Duplicate label: {label}')
        binaries[label] = inspect(Path(path))
    for label, path in args.link_map:
        if label not in binaries:
            parser.error(f'Link map label has no binary: {label}')
        binaries[label]['link_map'] = inspect_map(Path(path))
    report = {'schema': 1, 'binaries': binaries,
              'limitations': ['File bytes include appended payloads and signatures.',
                              'Virtual segments include zero-fill and are not file sizes.',
                              'Compile flags and functional equivalence require separate validation.',
                              'This audit does not measure runtime memory or performance.']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    for label, binary in binaries.items():
        print(f"{label}: {binary['file_bytes']} file bytes")


if __name__ == '__main__':
    main()
