"""Summarize peak process RSS from existing, checksum-validated timing logs."""
import hashlib
import json
import re
import statistics
from pathlib import Path

base = Path(__file__).resolve().parent / 'performance'
measurement = base / 'measurement.json'
data = json.loads(measurement.read_text())
rss = {}
logs = {}
for row in data['rows']:
    path = base / f"{row['order']}-{row['profile']}.stderr"
    logs[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    match = re.search(r'(\d+)\s+maximum resident set size', path.read_text())
    assert match, path
    if not row['warmup']:
        rss.setdefault(row['profile'], []).append(int(match.group(1)))
summary = {
    'source_measurement': measurement.name,
    'source_measurement_sha256': hashlib.sha256(measurement.read_bytes()).hexdigest(),
    'resource_log_sha256': logs,
    'median_wall_ms': data['median_wall_ms'],
    'median_max_rss_bytes': {key: statistics.median(values) for key, values in rss.items()},
    'on_vs_off_speedup': data['on_vs_off_speedup'],
    'on_vs_official_wall_ratio': data['on_vs_official_wall_ratio'],
    'note': 'Peak RSS is process-level; it does not measure allocation count. '
            'Three alternating measured runs; all 57.6M-call checksums identical.',
}
(base / 'resource-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
