"""Fresh own Pub -> native test integration, with independent upstream VM oracle.

Run from any directory. Work, Pub caches, compiler temporary artifacts and reports
stay on the SSD. No package configuration is copied from an official resolution.
The script performs correctness checks only and records every executed command.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
DEFAULT_PRODUCER = ROOT / 'target/constant-specialization-producer-v3/dartforge'
DEFAULT_SHA = 'c363ac60f6dd39338eb5dbd84dffb863767c1c8a2a2ae4c725c996b2847c4a79'
NAMES = {'ordinary sync', 'awaited future', 'unawaited timer error'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_versions(config):
    """Package configs supply name/paths/language; read actual package manifests."""
    value = json.loads(config.read_text())
    result = {}
    from urllib.parse import urljoin, urlparse, unquote
    for entry in value['packages']:
        uri = urljoin(config.as_uri(), entry['rootUri'])
        parsed = urlparse(uri)
        if parsed.scheme != 'file':
            raise AssertionError('package root must be a file URI')
        root = Path(unquote(parsed.path))
        result[entry['name']] = {'languageVersion': entry.get('languageVersion'),
                                  'pubspec_sha256': sha(root / 'pubspec.yaml'),
                                  'root': str(root)}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--producer', type=Path, default=DEFAULT_PRODUCER)
    parser.add_argument('--producer-sha256', default=DEFAULT_SHA)
    parser.add_argument('--sdk', type=Path, default=Path('/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk'))
    parser.add_argument('--work-root', type=Path, default=ROOT / 'target/native-pub-test-integration')
    options = parser.parse_args()
    cli = options.producer.resolve()
    sdk = options.sdk.resolve()
    assert sha(cli) == options.producer_sha256, 'frozen producer identity mismatch'
    work_root = options.work_root.resolve()
    assert str(work_root).startswith('/Volumes/ssd_dock/'), 'all work must stay on SSD'
    run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    work = work_root / run_id
    evidence = ARCHIVE / 'runs' / run_id
    work.mkdir(parents=True, exist_ok=False)
    evidence.mkdir(parents=True, exist_ok=False)
    fixtures = {str(p.relative_to(ARCHIVE)): sha(p) for p in (ARCHIVE / 'project').rglob('*') if p.is_file()}
    for mode in ['native', 'official']:
        shutil.copytree(ARCHIVE / 'project', work / mode / 'project')
        (work / mode / 'pub-cache').mkdir()
        (work / mode / 'tmp').mkdir()
    environment = {k: v for k, v in os.environ.items()
                   if not k.startswith('DARTFORGE_') and k not in ['PUB_CACHE', 'PUB_HOSTED_URL', 'INTEGRATION_FAILURE']}
    environment.update(PUB_HOSTED_URL='https://pub.dev', DARTFORGE_SDK_LIB=str(sdk / 'lib'),
                       DARTFORGE_CLANG='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64/bin/clang',
                       LLVM_SYS_221_PREFIX='/Volumes/ssd_dock/llvm/LLVM-22.1.8-macOS-ARM64')
    report = {'schema': 1, 'status': 'running', 'producer': str(cli),
              'producer_sha256': sha(cli), 'SDK': str(sdk), 'SDK_binary_sha256': sha(sdk / 'bin/dart'),
              'fixtures': fixtures, 'work': str(work), 'initial_pub_caches_empty': True,
              'commands': [], 'checks': {}, 'scope': 'Own fresh Pub resolution followed by own native test; independent official Pub/VM oracle. No timings or broad toolchain parity claim.'}

    def save():
        text = json.dumps(report, indent=2) + '\n'
        (evidence / 'validation.json').write_text(text)
        (work / 'validation.json').write_text(text)

    def run(label, command, mode, failure=''):
        env = dict(environment, PUB_CACHE=str(work / mode / 'pub-cache'),
                   TMPDIR=str(work / mode / 'tmp'), INTEGRATION_FAILURE=failure)
        item = {'label': label, 'command': list(map(str, command)),
                'cwd': str(work / mode / 'project'), 'PUB_CACHE': env['PUB_CACHE'],
                'TMPDIR': env['TMPDIR'], 'INTEGRATION_FAILURE': failure}
        report['commands'].append(item)
        save()
        print(label, flush=True)
        result = subprocess.run(item['command'], cwd=item['cwd'], env=env,
                                capture_output=True, text=True, timeout=900)
        for stream in ['stdout', 'stderr']:
            (evidence / (label + '.' + stream)).write_text(getattr(result, stream))
            item[stream + '_sha256'] = sha(evidence / (label + '.' + stream))
        item['exit'] = result.returncode
        save()
        return result

    def summary(result, native):
        events = []
        for line in result.stdout.splitlines():
            try:
                event = json.loads(line)
                if isinstance(event, dict):
                    events.append(event)
            except json.JSONDecodeError:
                pass
        if native:
            suites = [e for e in events if e.get('type') == 'suiteDone']
            assert len(suites) == 1, 'native completion report required; compile failure is not an executed test'
            value = suites[0]
            return {key: value[key] for key in ['total', 'passed', 'failed', 'skipped', 'success']}
        starts = {e['test']['id']: e['test']['name'] for e in events if e.get('type') == 'testStart'}
        selected = {test_id: name for test_id, name in starts.items() if name in NAMES}
        assert set(selected.values()) == NAMES, 'official VM must execute all ordinary fixture bodies'
        done = [e for e in events if e.get('type') == 'testDone' and e.get('testID') in selected]
        assert len(done) == 3
        terminal = [e for e in events if e.get('type') == 'done']
        assert len(terminal) == 1
        return {'total': 3, 'passed': sum(e['result'] == 'success' and not e.get('skipped', False) for e in done),
                'failed': sum(e['result'] != 'success' for e in done),
                'skipped': sum(bool(e.get('skipped', False)) for e in done),
                'success': terminal[0]['success']}

    try:
        resolved = {}
        for mode in ['official', 'native']:
            command = [cli, 'pub', 'get', '--sdk', sdk / 'lib'] if mode == 'native' else [sdk / 'bin/dart', 'pub', 'get']
            result = run(mode + '-pub-online', command, mode)
            resolved[mode] = result.returncode == 0
            report['checks'][mode + '_fresh_resolution_succeeded'] = resolved[mode]
            save()
            if not resolved[mode]:
                continue
            project = work / mode / 'project'
            config = project / '.dart_tool/package_config.json'
            packages = package_versions(config)
            assert all(name in packages for name in ['test', 'test_api', 'test_core'])
            # Dependencies may be hosted cache entries or selected SDK packages; never a reused global Pub cache.
            cache = work / mode / 'pub-cache'
            assert all(Path(value['root']).is_relative_to(cache) or Path(value['root']).is_relative_to(sdk)
                       for name, value in packages.items() if name != 'dartforge_pub_test_integration')
            report['checks'][mode + '_packages'] = packages
            save()
        normalize = lambda packages: {name: {k: v for k, v in row.items() if k != 'root'} for name, row in packages.items()}
        report['checks']['independent_package_graph_exact'] = (all(resolved.values()) and
            normalize(report['checks']['native_packages']) == normalize(report['checks']['official_packages']))
        save()
        for mode in ['official', 'native']:
            if not resolved[mode]:
                continue
            for failure in ['', 'assertion', 'async']:
                label = mode + '-test-' + (failure or 'pass')
                command = ([cli, 'test', '--reporter', 'json', '--sdk', sdk / 'lib', '--keep-artifacts']
                           if mode == 'native' else [sdk / 'bin/dart', 'test', '--reporter', 'json'])
                result = run(label, command, mode, failure)
                expected = {'total': 3, 'passed': 2 if failure else 3,
                            'failed': 1 if failure else 0, 'skipped': 0, 'success': not failure}
                expected['success'] = bool(expected['success'])
                actual = summary(result, mode == 'native')
                report['checks'][label] = {'actual': actual, 'expected': expected,
                                          'exact': actual == expected and result.returncode == (1 if failure else 0)}
                save()
                assert report['checks'][label]['exact'], label
                if failure:
                    marker = 'ASSERTION_INTEGRATION' if failure == 'assertion' else 'ASYNC_INTEGRATION'
                    assert marker in result.stdout + result.stderr, 'failure must arise from the intended executed test body'
            project = work / mode / 'project'
            files = [project / 'pubspec.lock', project / '.dart_tool/package_config.json']
            before = {str(path.relative_to(project)): sha(path) for path in files}
            package_graph_before = package_versions(project / '.dart_tool/package_config.json')
            command = [cli, 'pub', 'get', '--offline', '--sdk', sdk / 'lib'] if mode == 'native' else [sdk / 'bin/dart', 'pub', 'get', '--offline']
            result = run(mode + '-pub-offline', command, mode)
            assert result.returncode == 0
            after = {str(path.relative_to(project)): sha(path) for path in files}
            assert before['pubspec.lock'] == after['pubspec.lock']
            assert package_graph_before == package_versions(project / '.dart_tool/package_config.json')
            if mode == 'native':
                assert before == after
            report['checks'][mode + '_offline_artifacts'] = {
                'lock_byte_identical': True, 'package_graph_exact': True,
                'config_byte_identical': before['.dart_tool/package_config.json'] == after['.dart_tool/package_config.json'],
                'official_generated_timestamp_is_not_a_semantic_difference': mode == 'official'}
            command = ([cli, 'test', '--reporter', 'json', '--sdk', sdk / 'lib', '--keep-artifacts']
                       if mode == 'native' else [sdk / 'bin/dart', 'test', '--reporter', 'json'])
            result = run(mode + '-test-offline-pass', command, mode)
            assert result.returncode == 0 and summary(result, mode == 'native') == {
                'total': 3, 'passed': 3, 'failed': 0, 'skipped': 0, 'success': True}
        report['checks']['inputs_stable'] = sha(cli) == options.producer_sha256 and all(
            sha(ARCHIVE / name) == digest for name, digest in fixtures.items())
        assert report['checks']['inputs_stable']
        assert all(resolved.values()), 'fresh Pub resolution failed; successful VM reference tests do not prove native integration'
        assert report['checks']['independent_package_graph_exact'], 'independent package graph/language versions differ'
        report['status'] = 'passed'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = repr(error)
        save()
        raise
    save()
    print(str(evidence / 'validation.json'), flush=True)


if __name__ == '__main__':
    main()
