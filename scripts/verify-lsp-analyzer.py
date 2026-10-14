#!/usr/bin/env python3
"""Audit real stdio LSP sessions and analyzer output against an installed Dart SDK."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Session:
    def __init__(self, binary, environment, log):
        self.process = subprocess.Popen([str(binary), '--stdio'], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=log, env=environment)
        self.messages = queue.Queue()
        self.pending = []
        self.transcript = []
        self.next_id = 0
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        try:
            while True:
                headers = {}
                while True:
                    line = self.process.stdout.readline()
                    if not line:
                        self.messages.put(RuntimeError('LSP stdout closed'))
                        return
                    if line == b'\r\n':
                        break
                    key, value = line.decode('ascii').split(':', 1)
                    headers[key.lower()] = value.strip()
                length = int(headers['content-length'])
                body = b''
                while len(body) < length:
                    part = self.process.stdout.read(length - len(body))
                    if not part:
                        raise RuntimeError('Truncated LSP body')
                    body += part
                self.messages.put(json.loads(body))
        except Exception as error:
            self.messages.put(error)

    def send(self, method, params, request=False):
        message = {'jsonrpc': '2.0', 'method': method, 'params': params}
        if request:
            self.next_id += 1
            message['id'] = self.next_id
        self.transcript.append({'direction': 'client', 'message': message})
        body = json.dumps(message).encode()
        self.process.stdin.write(f'Content-Length: {len(body)}\r\n\r\n'.encode() + body)
        self.process.stdin.flush()
        return self.next_id if request else None

    def wait(self, predicate, timeout=120):
        end = time.monotonic() + timeout
        while True:
            for index, item in enumerate(self.pending):
                if predicate(item):
                    return self.pending.pop(index)
            remaining = end - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f'LSP wait expired; pending={self.pending!r}')
            item = self.messages.get(timeout=remaining)
            if isinstance(item, Exception):
                raise item
            self.transcript.append({'direction': 'server', 'message': item})
            self.pending.append(item)

    def request(self, method, params):
        ident = self.send(method, params, True)
        result = self.wait(lambda m: m.get('id') == ident and
                           ('result' in m or 'error' in m))
        if 'error' in result:
            raise RuntimeError(f'{method}: {result["error"]}')
        return result['result']

    def close(self):
        try:
            self.request('shutdown', None)
            self.send('exit', None)
            self.process.stdin.close()
            code = self.process.wait(timeout=10)
            if code:
                raise RuntimeError(f'LSP exit {code}')
        finally:
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'target/p4/release/dartforge')
    parser.add_argument('--lsp', type=Path, default=ROOT / 'target/p4/debug/dartforge-lsp')
    parser.add_argument('--dart', type=Path,
                        default=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/bin/dart'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--skip-lsp', action='store_true')
    parser.add_argument('--skip-analyzer', action='store_true')
    parser.add_argument('--explorer', type=Path,
                        default=Path('/Volumes/ssd_dock/MyProjectsDart/dart_ui/examples/file_explorer'))
    args = parser.parse_args()
    # Analyzer commands run inside each fixture's directory. Resolve tools at
    # invocation time so relative paths retain their original meaning.
    args.compiler = args.compiler.resolve()
    args.lsp = args.lsp.resolve()
    args.dart = args.dart.resolve()
    args.explorer = args.explorer.resolve()
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error('Output must be empty; preserve previous audit evidence')
    output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, TMPDIR=str(output),
               DARTFORGE_SDK_LIB=str(args.dart.resolve().parent.parent / 'lib'))
    for name in ('RUST_LOG', 'DARTFORGE_LSP_DEBUG'):
        env.pop(name, None)
    fixture = output / 'project'
    (fixture / 'lib').mkdir(parents=True)
    (fixture / 'pubspec.yaml').write_text('name: lsp_audit_fixture\nenvironment:\n  sdk: ">=3.6.0 <4.0.0"\n')
    (fixture / '.dart_tool').mkdir()
    (fixture / '.dart_tool/package_config.json').write_text(json.dumps({
        'configVersion': 2, 'packages': [{'name': 'lsp_audit_fixture',
        'rootUri': '../', 'packageUri': 'lib/', 'languageVersion': '3.6'}]}))
    helper = fixture / 'lib/helper.dart'
    helper.write_text("String greet(String name) => 'Hi $name';\n")
    main_file = fixture / 'lib/main.dart'
    source = "import 'helper.dart';\nvoid main() {\n  final value = greet('Ada');\n  print(value);\n}\n"
    main_file.write_text(source)
    ledger = {'schema': 1, 'sdk_lib': env['DARTFORGE_SDK_LIB'], 'lsp': {},
              'analyzer': [], 'production_ready_claim': False,
              'inputs': {str(p.resolve()): sha(p.resolve()) for p in
                         [Path(__file__), args.dart, args.compiler] + ([] if args.skip_lsp else [args.lsp])}}

    def save():
        (output / 'ledger.json').write_text(json.dumps(ledger, indent=2) + '\n')

    if not args.skip_lsp:
        transcript = output / 'lsp-transcript.json'
        with (output / 'lsp.stderr').open('wb') as log:
            session = Session(args.lsp.resolve(), env, log)
            try:
                initialized = session.request('initialize', {
                    'processId': os.getpid(), 'rootUri': fixture.as_uri(),
                    'workspaceFolders': [{'uri': fixture.as_uri(), 'name': 'fixture'}],
                    'capabilities': {'textDocument': {'rename': {'prepareSupport': True}}}})
                capabilities = initialized['capabilities']
                for key in ('completionProvider', 'definitionProvider', 'renameProvider'):
                    assert capabilities.get(key), (key, capabilities)
                session.send('initialized', {})
                uri = main_file.as_uri()
                session.send('textDocument/didOpen', {'textDocument': {
                    'uri': uri, 'languageId': 'dart', 'version': 1, 'text': source}})
                params = {'textDocument': {'uri': uri},
                          'position': {'line': 2, 'character': 17}}
                definition = session.request('textDocument/definition', params)
                assert definition['uri'] == helper.as_uri(), definition
                assert definition['range']['start'] == {'line': 0, 'character': 7}, definition
                rename = session.request('textDocument/rename', dict(params, newName='welcome'))
                changes = rename.get('changes', {})
                if not changes:
                    changes = {x['textDocument']['uri']: x['edits']
                               for x in rename.get('documentChanges', [])}
                assert set(changes) == {uri, helper.as_uri()}, rename
                assert sum(map(len, changes.values())) == 2, rename
                assert all(e['newText'] == 'welcome' for edits in changes.values() for e in edits), rename
                completion = session.request('textDocument/completion', {
                    'textDocument': {'uri': uri}, 'position': {'line': 2, 'character': 18},
                    'context': {'triggerKind': 1}})
                items = completion.get('items', []) if isinstance(completion, dict) else completion
                assert any(i['label'].startswith('greet') for i in items), completion
                sdk_source = "void main() {\n  final String value = 'Ada';\n  value.\n}\n"
                session.send('textDocument/didChange', {'textDocument': {'uri': uri, 'version': 2},
                    'contentChanges': [{'text': sdk_source}]})
                sdk_completion = session.request('textDocument/completion', {
                    'textDocument': {'uri': uri}, 'position': {'line': 2, 'character': 8},
                    'context': {'triggerKind': 2, 'triggerCharacter': '.'}})
                sdk_items = (sdk_completion.get('items', []) if isinstance(sdk_completion, dict)
                             else sdk_completion)
                assert any(i['label'].startswith('trim(') for i in sdk_items), sdk_completion
                sdk_definition = session.request('textDocument/definition', {
                    'textDocument': {'uri': uri}, 'position': {'line': 1, 'character': 9}})
                assert sdk_definition['uri'].endswith('/core/string.dart'), sdk_definition
                bad = 'void main() { if (1) {} }\n'
                session.send('textDocument/didChange', {'textDocument': {'uri': uri, 'version': 3},
                                                       'contentChanges': [{'text': bad}]})
                typed = session.wait(lambda m: m.get('method') == 'textDocument/publishDiagnostics'
                    and m['params'].get('uri') == uri and m['params'].get('version') == 3
                    and any(d.get('code') == 'non_bool_condition' for d in m['params']['diagnostics']))
                fixed = 'void main() {}\n'
                session.send('textDocument/didChange', {'textDocument': {'uri': uri, 'version': 4},
                    'contentChanges': [{'range': {'start': {'line': 0, 'character': 0},
                      'end': {'line': 1, 'character': 0}}, 'text': fixed}]})
                session.wait(lambda m: m.get('method') == 'textDocument/publishDiagnostics'
                    and m['params'].get('uri') == uri and m['params'].get('version') == 4
                    and not m['params']['diagnostics'])
                syntax = 'void main() { final value = ; }\n'
                session.send('textDocument/didChange', {'textDocument': {'uri': uri, 'version': 5},
                                                       'contentChanges': [{'text': syntax}]})
                invalid = session.wait(lambda m: m.get('method') == 'textDocument/publishDiagnostics'
                    and m['params'].get('uri') == uri and m['params'].get('version') == 5
                    and m['params']['diagnostics'])
                ledger['lsp'] = {'passed': True, 'capabilities': capabilities,
                    'definition': definition, 'rename': rename, 'typed_diagnostics': typed,
                    'syntax_diagnostics': invalid, 'sdk_definition': sdk_definition,
                    'sdk_completion_labels': [i['label'] for i in sdk_items],
                    'completion_labels': [i['label'] for i in items]}
                session.close()
            finally:
                transcript.write_text(json.dumps(session.transcript, indent=2) + '\n')
                if session.process.poll() is None:
                    session.process.kill()
                    session.process.wait()
                save()
    if not args.skip_analyzer:
        cases = {
            'valid_sdk': "import 'dart:async';\nimport 'dart:io';\nimport 'dart:convert';\nimport 'dart:typed_data';\nFuture<void> main() async { final bytes = Uint8List.fromList([65]); print(jsonEncode({'path': Directory.current.path, 'bytes': bytes})); await Future<void>.delayed(Duration.zero); }\n",
            'non_bool': 'void main() { if (1) {} }\n',
            'undefined': 'void main() { print(missing); }\n',
            'assignment': "void main() { int value = 'wrong'; print(value); }\n",
            'syntax': 'void main() { final value = ; print(value); }\n',
            'nullable_receiver': 'void f(String? value) { print(value.length); }\n',
            'argument_type': "void f(int value) {}\nvoid main() { f('wrong'); }\n",
            'return_type': "int f() => 'wrong';\n",
            'required_argument': 'void f({required int value}) {}\nvoid main() { f(); }\n',
            'invalid_override': "class A { int f() => 1; }\nclass B extends A { String f() => 'wrong'; }\n",
            'valid_records_patterns': 'int sum((int, int) pair) { final (a, b) = pair; return a + b; }\nvoid main() { print(sum((2, 3))); }\n',

        }
        targets = []
        for name, text in cases.items():
            path = fixture / 'lib' / (name + '.dart')
            # SDK 3.6.2 can reuse a cached contextual diagnostic URI across
            # identical source text in different temporary projects.
            path.write_text(text + '// Audit fixture: ' + output.name + '\n')
            targets.append((name, path))
        if args.explorer.exists():
            targets.append(('file_explorer', args.explorer / 'bin/file_explorer.dart'))
            targets.append(('explorer_model', args.explorer / 'lib/src/engine/explorer_model.dart'))
            targets.append(('framework_application', args.explorer.parent.parent / 'lib/src/app/application.dart'))
        for label, path in targets:
            reports = {}
            statuses = {}
            for variant, command in (
                ('official', [args.dart, 'analyze', '--format=json', path]),
                ('dartforge', [args.compiler, 'analyze', '--format=json', path])):
                result = subprocess.run(list(map(str, command)), cwd=path.parent, env=env,
                                        capture_output=True, timeout=600)
                (output / f'{label}-{variant}.stdout').write_bytes(result.stdout)
                (output / f'{label}-{variant}.stderr').write_bytes(result.stderr)
                statuses[variant] = result.returncode
                reports[variant] = json.loads(result.stdout)
            official = reports['official']['diagnostics']
            actual = reports['dartforge']['diagnostics']
            ledger['analyzer'].append({'case': label, 'path': str(path), 'source_sha256': sha(path),
                'exit_codes': statuses, 'exact_match': reports['official'] == reports['dartforge'],
                'official': official, 'dartforge': actual,
                'missing': [d for d in official if d not in actual],
                'additional': [d for d in actual if d not in official]})
            if reports['official'] != reports['dartforge']:
                result = subprocess.run([str(args.compiler), 'analyze', '--format=json', '--all',
                                         str(path)], cwd=path.parent, env=env,
                                        capture_output=True, timeout=600)
                (output / f'{label}-dartforge-all.stdout').write_bytes(result.stdout)
                (output / f'{label}-dartforge-all.stderr').write_bytes(result.stderr)
                unfiltered = json.loads(result.stdout)
                ledger['analyzer'][-1]['unfiltered'] = {
                    'exit_code': result.returncode,
                    'exact_match': unfiltered == reports['official'], 'report': unfiltered}
            save()
            print(label + (' EXACT' if reports['official'] == reports['dartforge'] else ' DIFF'), flush=True)
    ledger['inputs_stable'] = all(sha(Path(path)) == digest
                                  for path, digest in ledger['inputs'].items())
    ledger['audit_completed'] = True
    ledger['analyzer_exact_match_count'] = sum(case['exact_match']
                                              for case in ledger['analyzer'])
    save()
    if not ledger['inputs_stable']:
        raise RuntimeError('Audit executable or runner changed during validation')


if __name__ == '__main__':
    main()
