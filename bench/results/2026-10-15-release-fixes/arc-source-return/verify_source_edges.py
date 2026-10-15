"""Check that each sabotage removes exactly its intended emitted source edge.

Run from the repository root after scripts/prove-arc-aot.py arc_source_return.
"""
import json
import re
from pathlib import Path

root = Path.cwd()
base = root / 'target/proof-source-return'


def body(ir, name):
    match = re.search(r'^define [^\n]*@[^\n(]*' + re.escape(name) + r'\([^\n]*\{\n.*?^}\n', ir, re.M | re.S)
    assert match, name
    return re.sub(r'@df\.[^\s(]*\.\.(identidade|repassar)', r'@SOURCE..\1', match.group())


checks = []
for mode in ('arc', 'tracing'):
    for level in (0, 2):
        stem = f'{mode}-O{level}-normal'
        positive = (base / (stem + '.ll')).read_text()
        retain = 'df.arc_retain_ref' if 'define internal void @df.arc_retain_ref(' in positive else 'dartforge_arc_retain'
        release = 'df.arc_release_ref' if 'define internal void @df.arc_release_ref(' in positive else 'dartforge_arc_release'
        for control, name, target in (
            ('no-retain', 'identidade', f'  call void @{retain}(i64 %v0)\n'),
            ('no-intermediate-drop', 'repassar', f'  call void @{release}(i64 %v4)\n'),
        ):
            changed = (base / (stem + '-' + control + '.ll')).read_text()
            before, after = body(positive, name), body(changed, name)
            assert before.count(target) == 1, (stem, control, target)
            assert before.replace(target, '') == after, (stem, control)
            checks.append({'mode': mode, 'level': level, 'control': control, 'removed': target.strip()})
print(json.dumps({'checked_edges': len(checks), 'checks': checks}, indent=2))
