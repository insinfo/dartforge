"""Inspect actual selected SDK .poda7 transport; this does not certify import pruning."""
import argparse
import hashlib
import json
from pathlib import Path


def inspect(directory):
    result = {}
    for path in sorted(directory.glob('*.poda')):
        source = path.read_text()
        lines = source.splitlines()
        header = lines[0].split('\t')
        if header[:2] != ['dartforge-poda', '7']:
            raise ValueError(f'{path}: unexpected pruning summary version')
        names = [line.split('\t', 1)[1] for line in lines if line.startswith('N\t')]
        definitions = {int(line.split('\t')[1]) for line in lines if line.startswith('D\t')}
        audits = {}
        for line in lines:
            if not line.startswith('A\t'):
                continue
            fields = line.split('\t')
            if len(fields) != 4:
                raise ValueError(f'{path}: malformed allocation record')
            node = int(fields[1])
            if node not in definitions or node in audits or not 0 <= node < len(names):
                raise ValueError(f'{path}: duplicate or nondefinition allocation record')
            kind, payload = fields[2:]
            if kind == 'unknown' and not payload:
                audits[node] = None
            elif kind == 'known':
                classes = [int(word) for word in payload.split()]
                if len(set(classes)) != len(classes) or any(not 0 <= cid <= 0xffffffff for cid in classes):
                    raise ValueError(f'{path}: invalid allocated CID')
                audits[node] = classes
            else:
                raise ValueError(f'{path}: invalid allocation state')
        missing = sorted(definitions - set(audits))
        result[path.name] = {
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'bytes': path.stat().st_size,
            'module': header[2],
            'definition_nodes': len(definitions),
            'audit_records': len(audits),
            'known_empty': sum(value == [] for value in audits.values()),
            'known_nonempty': sum(value is not None and bool(value) for value in audits.values()),
            'explicit_unknown': sum(value is None for value in audits.values()),
            'missing_unknown': len(missing),
            'known_empty_examples': [names[node] for node, value in audits.items() if value == []][:5],
            'all_definitions_have_explicit_record': not missing,
        }
    if not result:
        raise ValueError('No selected SDK .poda files found')
    marker = directory / 'pronto'
    return {
        'directory': str(directory.resolve()),
        'marker': json.loads(marker.read_text()) if marker.is_file() else None,
        'marker_sha256': hashlib.sha256(marker.read_bytes()).hexdigest() if marker.is_file() else None,
        'summaries': result,
        'total_known_empty': sum(row['known_empty'] for row in result.values()),
        'total_unknown': sum(row['explicit_unknown'] + row['missing_unknown'] for row in result.values()),
        'scope': 'Transport syntax and actual artifact hashes only. Known-empty means directly audited scalar leaf; transitive/runtime/native completeness and import acceptance are not certified here.',
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('selected_sdk_directory', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    options = parser.parse_args()
    report = inspect(options.selected_sdk_directory)
    options.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'summaries': len(report['summaries']), 'known_empty': report['total_known_empty'], 'unknown': report['total_unknown']}))
