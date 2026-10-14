import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root = Path('/Volumes/ssd_dock/MyRustProjects/dartforge')
ui = Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui')
binary = Path(sys.argv[1]).resolve(strict=True)
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=False)
fixture = root / 'target/file-explorer-fixture'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
manifest = {'binary': str(binary), 'binary_sha256': sha(binary),
            'binary_bytes': binary.stat().st_size, 'cwd': str(ui),
            'args': [str(fixture), '--lang=en'], 'actions': [],
            'visual_review_completed': False, 'performance_claim': False}
with (output / 'stdout').open('wb') as stdout, (output / 'stderr').open('wb') as stderr:
    proc = subprocess.Popen([str(binary), *manifest['args']], cwd=ui,
                            env=dict(os.environ, TMPDIR=str(output)), stdout=stdout, stderr=stderr)
    manifest['pid'] = proc.pid
    time.sleep(3)
    for action in ('capture', 'navigate', 'back-button', 'close-button'):
        command = [sys.executable, str(root / 'target/native-provenance/targeted_window_control.py'),
                   '--pid', str(proc.pid), '--output', str(output), '--action', action]
        if action == 'navigate':
            command += ['--folder', str(fixture / 'dartforge-child')]
        result = subprocess.run(command, capture_output=True, timeout=30)
        manifest['actions'].append({'action': action, 'exit_code': result.returncode,
                                    'stdout': result.stdout.decode(), 'stderr': result.stderr.decode()})
        (output / 'validation.json').write_text(json.dumps(manifest, indent=2) + '\n')
        if result.returncode:
            manifest['poll_at_action_failure'] = proc.poll()
            (output / 'validation.json').write_text(json.dumps(manifest, indent=2) + '\n')
            print(f'FAILED_ACTION={action} APP_PID={proc.pid} EXIT={proc.poll()}', flush=True)
            # Keep the owning process alive for independent window inspection.
            # Returning this runner immediately can terminate its process group.
            manifest['exit_after_action_failure'] = proc.wait()
            (output / 'validation.json').write_text(json.dumps(manifest, indent=2) + '\n')
            raise RuntimeError(f'UI action failed: {action}; inspect preserved evidence')
    manifest['exit_code'] = proc.wait(timeout=15)
manifest['stdout_bytes'] = (output / 'stdout').stat().st_size
manifest['stderr_bytes'] = (output / 'stderr').stat().st_size
manifest['binary_unchanged'] = sha(binary) == manifest['binary_sha256']
manifest['screenshots'] = {p.name: sha(p) for p in output.glob('*.png')}
(output / 'validation.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(manifest, indent=2), flush=True)
if manifest['exit_code'] or manifest['stdout_bytes'] or manifest['stderr_bytes'] or not manifest['binary_unchanged']:
    raise RuntimeError('Native application exit/output/hash validation failed')
