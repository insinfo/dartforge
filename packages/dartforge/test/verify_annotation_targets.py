#!/usr/bin/env python3
"""Verify portable analyzer target feedback without installing a verifier."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.parse import urljoin


def main():
    package = Path(__file__).resolve().parents[1]
    config_path = package / '.dart_tool' / 'package_config.json'
    config = json.loads(config_path.read_text())
    for entry in config['packages']:
        entry['rootUri'] = urljoin(config_path.as_uri(), entry['rootUri'])
    # Repository target stays on the same SSD as the checkout, even when the
    # shell does not set TMPDIR. Fixtures do not enter the package's source tree.
    scratch = package.parents[1] / 'target' / 'tmp-annotation-targets'
    scratch.mkdir(parents=True, exist_ok=True)
    cases = {
        'region_class': '@PreferRegion() class Incorrect {}',
        'escape_function': '@NoEscape() void incorrect() {}',
        'realtime_variable': '@Realtime() const incorrect = 1;',
    }
    with tempfile.TemporaryDirectory(prefix='targets-', dir=scratch) as directory:
        root = Path(directory)
        (root / '.dart_tool').mkdir()
        (root / '.dart_tool' / 'package_config.json').write_text(json.dumps(config))
        for label, declaration in cases.items():
            fixture = root / f'{label}.dart'
            fixture.write_text("import 'package:dartforge/dartforge.dart';\n" +
                               declaration + '\nvoid main() {}\n')
            result = subprocess.run(
                [os.environ.get('DART', 'dart'), 'analyze', '--format', 'machine', str(fixture)],
                capture_output=True, text=True, timeout=60)
            diagnostics = result.stdout + result.stderr
            if result.returncode == 0 or 'INVALID_ANNOTATION_TARGET' not in diagnostics:
                raise RuntimeError(f'{label}: expected target diagnostic\n{diagnostics}')
            if 'URI_DOES_NOT_EXIST' in diagnostics or 'UNDEFINED_' in diagnostics:
                raise RuntimeError(f'{label}: dependency resolution failed\n{diagnostics}')
    print('annotation analyzer targets validated: 3 invalid targets diagnosed')


if __name__ == '__main__':
    main()
