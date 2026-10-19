#!/usr/bin/env python3
"""Exercise the real CLI against a temporary loopback registry and cache."""
import argparse
import copy
import hashlib
import http.server
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import threading


def digest(data):
    return hashlib.sha256(data).hexdigest()


def snapshot(root):
    return {str(p.relative_to(root)): digest(p.read_bytes())
            for p in sorted(root.rglob('*')) if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--sdk-lib', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    options = parser.parse_args()
    compiler = options.compiler.resolve()
    sdk = options.sdk_lib.resolve()
    records = []
    with tempfile.TemporaryDirectory(prefix='dartforge-pub-cache-') as directory:
        root = Path(directory)
        project = root / 'project with spaces'
        project.mkdir()
        (project / 'pubspec.yaml').write_text(
            'name: app\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\n'
            'dependencies:\n  a: ^1.0.0\n')
        package = dict(name='a', version='1.0.0', environment={'sdk': '^3.0.0'})
        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode='w:gz') as tar:
            for name, data in [('pubspec.yaml', json.dumps(package).encode()),
                               ('lib/a.dart', b'const value = 1;\n')]:
                info = tarfile.TarInfo(name)
                info.size, info.mode, info.mtime = len(data), 0o644, 0
                tar.addfile(info, io.BytesIO(data))
        payload = archive.getvalue()
        requests = []
        current = {}

        class Registry(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                body = (json.dumps(current['value']).encode()
                        if self.path == '/api/packages/a' else payload)
                if self.path not in ['/api/packages/a', '/archive.tar.gz']:
                    self.send_error(404)
                    return
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.send_header('Connection', 'close')
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Registry)
        host = 'http://127.0.0.1:' + str(server.server_port)
        good = dict(name='a', versions=[dict(version='1.0.0', pubspec=package,
                    archive_url=host + '/archive.tar.gz', archive_sha256=digest(payload))])
        current['value'] = good
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        environment = dict(os.environ, PUB_CACHE=str(root / 'cache'), PUB_HOSTED_URL=host,
                           NO_PROXY='127.0.0.1,localhost', no_proxy='127.0.0.1,localhost')

        def run(label, offline=False, expected=0):
            command = [str(compiler), 'pub', 'get', '--directory', str(project),
                       '--sdk', str(sdk), '--sdk-version', '3.6.2']
            if offline:
                command.append('--offline')
            result = subprocess.run(command, env=environment, capture_output=True, text=True,
                                    timeout=30)
            records.append(dict(case=label, command=command, exit=result.returncode,
                                stdout=result.stdout, stderr=result.stderr))
            assert result.returncode == expected, records[-1]

        try:
            run('initial-online-resolution')
            assert requests == ['/api/packages/a', '/archive.tar.gz'], requests
            original_cache = snapshot(root / 'cache')
            artifacts = ['pubspec.lock', '.dart_tool/package_config.json']
            original_outputs = {p: (project / p).read_bytes() for p in artifacts}
            cases = ['package-name', 'missing-versions', 'invalid-version', 'duplicate-version',
                     'pubspec-name', 'pubspec-version', 'archive-digest', 'archive-transport']
            for case in cases:
                bad = copy.deepcopy(good)
                row = bad['versions'][0]
                if case == 'package-name':
                    bad['name'] = 'different'
                elif case == 'missing-versions':
                    del bad['versions']
                elif case == 'invalid-version':
                    row['version'] = 'invalid'
                elif case == 'duplicate-version':
                    bad['versions'].append(copy.deepcopy(row))
                elif case == 'pubspec-name':
                    row['pubspec']['name'] = 'different'
                elif case == 'pubspec-version':
                    row['pubspec']['version'] = '2.0.0'
                elif case == 'archive-digest':
                    row['archive_sha256'] = 'invalid'
                elif case == 'archive-transport':
                    row['archive_url'] = 'file:///archive.tar.gz'
                current['value'] = bad
                run('reject-' + case, expected=1)
                assert snapshot(root / 'cache') == original_cache, case
                assert all((project / p).read_bytes() == data
                           for p, data in original_outputs.items()), case
                before = len(requests)
                run('offline-after-' + case, offline=True)
                assert len(requests) == before, case
                assert snapshot(root / 'cache') == original_cache, case
                assert all((project / p).read_bytes() == data
                           for p, data in original_outputs.items()), case
            configuration = json.loads((project / '.dart_tool/package_config.json').read_text())
            assert {p['name'] for p in configuration['packages']} == {'a', 'app'}
            final_cache = snapshot(root / 'cache')
            output_hashes = {p: digest(data) for p, data in original_outputs.items()}
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
            assert not thread.is_alive()
    report = dict(compiler=str(compiler), compiler_sha256=digest(compiler.read_bytes()),
                  cases=records, passed=len(records), network='loopback only',
                  initial_cache_snapshot=original_cache, final_cache_snapshot=final_cache,
                  project_output_hashes=output_hashes, http_requests=requests)
    options.out.parent.mkdir(parents=True, exist_ok=True)
    options.out.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(passed=report['passed'], compiler_sha256=report['compiler_sha256'])))


if __name__ == '__main__':
    main()
