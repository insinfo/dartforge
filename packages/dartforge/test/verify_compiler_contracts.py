#!/usr/bin/env python3
"""Check native annotation policy and provenance, separately from proof claims."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_digest(package):
    hasher = hashlib.sha256()
    files = [package / 'pubspec.yaml', package / 'pubspec.lock'] + sorted((package / 'lib').rglob('*.dart'))
    for path in files:
        hasher.update(path.relative_to(package).as_posix().encode() + b'\0')
        hasher.update(path.read_bytes() + b'\0')
    return hasher.hexdigest()


def main():
    package = Path(__file__).resolve().parents[1]
    repository = package.parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=repository / 'target/p4/release/dartforge')
    parser.add_argument('--dart', default=os.environ.get('DART', 'dart'))
    parser.add_argument('--output', type=Path, default=repository / 'target/tmp-contracts-native')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    compiler = args.compiler.resolve()
    config = package / '.dart_tool/package_config.json'
    probe = package / 'test/annotations_probe.dart'
    environment = os.environ.copy()
    environment.update(TMPDIR=str(output), DARTFORGE_EFFECTS='check', DARTFORGE_ARC_CHECK='1')
    environment.pop('DARTFORGE_GC_STRESS', None)
    environment.pop('DARTFORGE_ARC_PREPARED', None)
    results = []

    def run(command, label, env=None, expect_success=True):
        completed = subprocess.run([str(v) for v in command], env=env or environment,
                                   capture_output=True, timeout=300)
        (output / (label + '.stdout')).write_bytes(completed.stdout)
        (output / (label + '.stderr')).write_bytes(completed.stderr)
        record = {'label': label, 'command': [str(v) for v in command], 'exit': completed.returncode}
        results.append(record)
        if expect_success and completed.returncode:
            raise RuntimeError(f'{label}: command failed\n{completed.stderr.decode(errors="replace")}')
        return completed

    def compile_command(source, artifact, memory='tracing', policy=None, emit_ir=False):
        command = [compiler, 'compile-native', source, '--packages', config, '--optimize',
                   '--exceptions', 'tables', '--roots', 'shadow', '--stack-trace', 'none',
                   '--memory', memory, '-o', artifact]
        if policy:
            command.append('--dartforge-contracts=' + policy)
        if emit_ir:
            command.append('--emit-ir')
        return command

    def manifest(artifact, policy, obligations):
        path = Path(str(artifact) + '.contracts.json')
        value = json.loads(path.read_text())
        assert value['mode'] == policy and value['certified'] is False
        assert value['verification_implemented'] is False
        assert value['package']['defining_uri'] == 'package:dartforge/src/annotations.dart'
        assert value['package']['annotation_schema'] == 1
        assert value['package']['sources'] and all(s['blake3'] for s in value['package']['sources'])
        assert value['sources'] and all(s['blake3'] for s in value['sources'])
        assert value['requests']
        actual_obligations = 0
        for request in value['requests']:
            assert request['certified'] is False
            assert request['span']['end'] > request['span']['start']
            if request['kind'].startswith('Preference('):
                assert request['status'] == 'not_applied'
            else:
                actual_obligations += 1
                assert request['status'] == 'ignored'
        assert actual_obligations == obligations
        return value

    reference = run([args.dart, '--packages=' + str(config), 'run', probe], 'annotations-vm').stdout
    official = output / 'annotations-official'
    run([args.dart, 'compile', 'exe', '--packages=' + str(config), probe, '-o', official], 'annotations-official-build')
    assert run([official], 'annotations-official').stdout == reference

    rejected_artifact = output / 'annotations-default-rejected'
    rejected = run(compile_command(probe, rejected_artifact), 'annotations-default-verify', expect_success=False)
    diagnostic = (rejected.stdout + rejected.stderr).decode(errors='replace')
    assert rejected.returncode != 0 and 'NoEscape' in diagnostic and 'Realtime' in diagnostic
    assert len(re.findall(r'annotations_probe\.dart:\d+:\d+', diagnostic)) >= 4
    assert not rejected_artifact.exists() and not Path(str(rejected_artifact) + '.contracts.json').exists()

    manifests = {}
    for mode in ['tracing', 'arc', 'prepared']:
        env = environment.copy()
        if mode == 'prepared':
            env['DARTFORGE_ARC_PREPARED'] = '1'
        artifact = output / ('annotations.' + mode)
        run(compile_command(probe, artifact, 'tracing' if mode == 'tracing' else 'arc', 'ignore'), mode + '-build', env)
        manifests[mode] = manifest(artifact, 'ignore', 4)
        for stress in [False, True]:
            run_environment = env.copy()
            if stress:
                run_environment['DARTFORGE_GC_STRESS'] = '1'
            label = mode + ('-stress' if stress else '-normal')
            executed = run([artifact], label, run_environment)
            assert executed.stdout == reference and not executed.stderr
            results[-1].update(matches_vm=True, stderr_empty=True)
            print(label + ': passed', flush=True)

    sidecars = [Path(str(output / ('annotations.' + mode)) + '.contracts.json')
                for mode in ['tracing', 'arc', 'prepared']]
    assert len(set(sidecars)) == 3 and all(path.exists() for path in sidecars)
    for mode, path in zip(['tracing', 'arc', 'prepared'], sidecars):
        assert manifests[mode]['artifact']['path'] == str(output / ('annotations.' + mode))

    ir = output / 'annotations-ignore.ll'
    run(compile_command(probe, ir, policy='ignore', emit_ir=True), 'annotations-ignore-ir')
    ir_report = manifest(ir, 'ignore', 4)
    assert ir_report['artifact']['kind'] == 'llvm_ir'
    assert ir_report['artifact']['ir_digest']

    preference = output / 'preferences.dart'
    preference.write_text("""import 'package:dartforge/dartforge.dart';
@PreferAcyclic() final class Number { @PreferAcyclic() const Number(this.value); final int value; }
@PreferRegion() int doubleValue(Number number) => number.value * 2;
void main() { @PreferAcyclic() const number = Number(3); print(doubleValue(number)); }
""")
    preference_reference = run([args.dart, '--packages=' + str(config), 'run', preference], 'preferences-vm').stdout
    preference_artifact = output / 'preferences-default'
    run(compile_command(preference, preference_artifact), 'preferences-default-build')
    manifest(preference_artifact, 'verify', 0)
    assert run([preference_artifact], 'preferences-default-run').stdout == preference_reference
    cache_tags = {}
    for policy in ['verify', 'ignore']:
        artifact = output / ('preferences-' + policy + '.ll')
        run(compile_command(preference, artifact, policy=policy, emit_ir=True), 'preferences-' + policy + '-ir')
        manifest(artifact, policy, 0)
        cache_tags[policy] = artifact.read_text().splitlines()[0]
    assert cache_tags['verify'].startswith('; DartForge contract provenance ')
    assert cache_tags['verify'] != cache_tags['ignore']
    assert (output / 'preferences-verify.ll').read_text().splitlines()[1:] == (output / 'preferences-ignore.ll').read_text().splitlines()[1:]

    report = {'compiler_sha256': digest(compiler), 'probe_sha256': digest(probe),
              'package_sha256': package_digest(package),
              'annotation_source_sha256': digest(package / 'lib/src/annotations.dart'),
              'package_config_sha256': digest(config),
              'sdk': run([args.dart, '--version'], 'sdk-version').stdout.decode().strip(),
              'reference_stdout': reference.decode(), 'commands': results,
              'manifests': manifests, 'ir_manifest': ir_report,
              'preference_cache_tags': cache_tags, 'sidecars': [str(path) for path in sidecars], 'certified': False}
    (output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print('compiler contract policy and uncertified provenance validated', flush=True)


if __name__ == '__main__':
    main()
