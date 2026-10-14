"""Validate prebuilt UTF-8 HTTP servers without treating elapsed time as evidence."""

import argparse
import hashlib
import http.client
import json
from pathlib import Path
import re
import selectors
import subprocess


def validate(path):
    server = subprocess.Popen([str(path), '0'], stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True)
    connection = None
    try:
        with selectors.DefaultSelector() as ready:
            ready.register(server.stdout, selectors.EVENT_READ)
            if not ready.select(15):
                raise TimeoutError('Server startup exceeded 15 seconds')
        startup = server.stdout.readline().strip()
        match = re.fullmatch(r'port (\d+)', startup)
        assert match, startup
        connection = http.client.HTTPConnection('127.0.0.1', int(match[1]), timeout=15)
        counter = 0
        for route in ['/', '/json', '/json', '/'] * 25:
            connection.request('GET', route)
            response = connection.getresponse()
            body = response.read()
            assert response.status == 200, (route, response.status, body)
            if route == '/json':
                counter += 1
                expected = {'message': 'Hello, World!', 'id': counter,
                            'active': True, 'items': [1, 2, 3]}
                assert body == json.dumps(expected, separators=(',', ':')).encode(), body
            else:
                assert body == b'hello', body
        connection.request('GET', '/quit')
        response = connection.getresponse()
        assert response.status == 200
        assert response.read() == b'bye'
        connection.close()
        connection = None
        stdout, stderr = server.communicate(timeout=15)
        assert server.returncode == 0, (server.returncode, stderr)
        assert stdout == '' and stderr == '', (stdout, stderr)
        return {'executable': str(path),
                'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                'bytes': path.stat().st_size, 'requests': 100, 'json_responses': counter,
                'bodies_exact': True, 'exit': server.returncode,
                'remaining_stdout_empty': True, 'stderr_empty': True}
    finally:
        if connection:
            connection.close()
        if server.poll() is None:
            server.kill()
            server.communicate()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executables', nargs='+', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    results = [validate(path.resolve(strict=True)) for path in args.executables]
    args.output.write_text(json.dumps(results, indent=2) + '\n')
