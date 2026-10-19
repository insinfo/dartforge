"""Check archive detection, caller overrides, response files and SDK choices."""
import importlib.util
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'rust_linker_macos', Path(__file__).with_name('rust-linker-macos.py'))
linker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(linker)


def member(name, payload, extended=False):
    if extended:
        encoded = name.encode()
        name, payload = f'#1/{len(encoded)}', encoded + payload
    header = f'{name:<16}{0:<12}{0:<6}{0:<6}{0:<8}{len(payload):<10}`\n'.encode()
    assert len(header) == 60
    return header + payload + (b'\n' if len(payload) % 2 else b'')


class RustLinkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='linker paths ')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.archive = self.root / 'libdartforge_llvm-test.rlib'
        self.archive.write_bytes(b'!<arch>\n' + member('metadata', b'MachO') +
                                 member('a very long LLVM member.o', b'\xde\xc0\x17\x0bmore', True))
        self.calls = []

    def prepared(self, environment, run):
        self.calls.append(dict(environment))
        if environment.get('SDKROOT') == '/explicit/bad.sdk':
            raise ValueError('No compatible macOS SDK')
        return dict(environment, DARTFORGE_LLVM_DIR='/LLVM with spaces',
                    SDKROOT=environment.get('SDKROOT', '/compatible/SDK with spaces'))

    def fake_run(self, arguments, **kwargs):
        self.assertEqual(arguments, ['xcrun', '--show-sdk-path'])
        return subprocess.CompletedProcess(arguments, 0, '/active/default.sdk\n', '')

    def test_native_and_bitcode_members_in_bsd_and_gnu_archives(self):
        self.assertTrue(linker.contains_bitcode(self.archive))
        for magic in linker.BITCODE_MAGIC:
            self.archive.write_bytes(b'!<arch>\n' + member('/0', magic + b'data'))
            self.assertTrue(linker.contains_bitcode(self.archive))
        self.archive.write_bytes(b'!<arch>\n' + member('metadata', b'MachO'))
        self.assertFalse(linker.contains_bitcode(self.archive))

    def test_malformed_archives_are_not_silently_classified_as_native(self):
        for data in (b'broken', b'!<arch>\nshort',
                     b'!<arch>\n' + member('file', b'payload')[:-4],
                     b'!<arch>\n' + member('#1/99', b'x')):
            self.archive.write_bytes(data)
            with self.assertRaises(ValueError):
                linker.contains_bitcode(self.archive)

    def test_ordinary_and_shared_llvm_links_keep_system_cc(self):
        for arguments in (['object.o', '-o', 'program'],
                          [str(self.archive), '-fuse-ld=/chosen/linker'],
                          [str(self.archive), '-Wl,-ld_classic']):
            command, environment = linker.command(arguments, {'SDKROOT': 'untouched'},
                                                   self.fake_run, self.prepared)
            self.assertEqual(command, ['cc', *arguments])
            self.assertEqual(environment, {'SDKROOT': 'untouched'})
        self.archive.write_bytes(b'!<arch>\n' + member('native.o', b'MachO'))
        command, _ = linker.command([str(self.archive), '-lLLVM'], {}, self.fake_run, self.prepared)
        self.assertEqual(command[0], 'cc')
        self.assertEqual(self.calls, [])

    def test_bitcode_link_preserves_original_arguments_and_selects_sdk(self):
        arguments = [str(self.archive), '-o', 'output with spaces', '-isysroot', '/active/default.sdk']
        original = {}
        command, environment = linker.command(arguments, original, self.fake_run, self.prepared)
        self.assertEqual(command[0], '/LLVM with spaces/bin/clang')
        self.assertEqual(command[1:1 + len(arguments)], arguments)
        self.assertEqual(command[-3:], ['-isysroot', '/compatible/SDK with spaces',
                                       '-fuse-ld=/LLVM with spaces/bin/ld64.lld'])
        self.assertEqual(original, {})
        self.assertIn('SDKROOT', environment)

    def test_unbundled_static_library_search_and_shared_preference(self):
        archive = self.root / 'libLLVMCore.a'
        archive.write_bytes(self.archive.read_bytes())
        for options in (['-L', str(self.root), '-lLLVMCore'],
                        ['-L' + str(self.root), '-l', 'LLVMCore']):
            command, _ = linker.command(options, {}, self.fake_run, self.prepared)
            self.assertEqual(command[0], '/LLVM with spaces/bin/clang')
        (self.root / 'libLLVMCore.dylib').touch()
        command, _ = linker.command(['-L', str(self.root), '-lLLVMCore'], {},
                                    self.fake_run, self.prepared)
        self.assertEqual(command[0], 'cc')

    def test_explicit_sdk_is_preserved_and_invalid_sdk_is_rejected(self):
        for arguments, environment in [([str(self.archive)], {'SDKROOT': '/explicit/bad.sdk'}),
                                       ([str(self.archive), '-Wl,-syslibroot,/explicit/bad.sdk'], {}),
                                       ([str(self.archive), '-Xlinker', '-syslibroot', '-Xlinker', '/explicit/bad.sdk'], {}),
                                       ([str(self.archive), '-isysroot', '/explicit/bad.sdk'], {})]:
            with self.assertRaisesRegex(ValueError, 'No compatible macOS SDK'):
                linker.command(arguments, environment, self.fake_run, self.prepared)

        with self.assertRaisesRegex(ValueError, 'conflicts'):
            linker.command([str(self.archive), '-isysroot', '/other.sdk'],
                           {'SDKROOT': '/configured.sdk'}, self.fake_run, self.prepared)

    def test_response_file_contents_are_inspected_but_not_rewritten(self):
        response = self.root / 'link arguments.txt'
        contents = shlex.join([str(self.archive), '-o', 'output with spaces'])
        response.write_text(contents)
        arguments = ['@' + str(response)]
        command, _ = linker.command(arguments, {}, self.fake_run, self.prepared)
        self.assertEqual(command[1], arguments[0])
        self.assertEqual(response.read_text(), contents)
        response.write_text(shlex.join([str(self.archive), '-fuse-ld=/explicit/linker']))
        command, _ = linker.command(arguments, {}, self.fake_run, self.prepared)
        self.assertEqual(command, ['cc', *arguments])
        response.write_text(shlex.join(['@' + str(response)]))
        with self.assertRaisesRegex(ValueError, 'Recursive'):
            linker.expand_response_files(arguments)

    def test_rustc_default_sdk_is_negotiated_but_a_project_pin_is_authoritative(self):
        arguments = [str(self.archive)]
        command, _ = linker.command(arguments, {'SDKROOT': '/active/default.sdk'},
                                    self.fake_run, self.prepared)
        self.assertEqual(command[-2], '/compatible/SDK with spaces')
        command, _ = linker.command(arguments,
                                    {'SDKROOT': '/active/default.sdk',
                                     'DARTFORGE_MACOS_SDK_ROOT': '/active/default.sdk'},
                                    self.fake_run, self.prepared)
        self.assertEqual(command[-2], '/active/default.sdk')
        command, _ = linker.command([str(self.archive), '-isysroot', '/custom/sdk'],
                                    {'SDKROOT': '/active/default.sdk'}, self.fake_run, self.prepared)
        self.assertEqual(command[-2], '/custom/sdk')
        with self.assertRaisesRegex(ValueError, 'No compatible macOS SDK'):
            linker.command(arguments, {'DARTFORGE_MACOS_SDK_ROOT': '/explicit/bad.sdk'},
                           self.fake_run, self.prepared)
        with self.assertRaisesRegex(ValueError, 'conflicts with SDKROOT'):
            linker.command(arguments, {'SDKROOT': '/custom/sdk',
                                       'DARTFORGE_MACOS_SDK_ROOT': '/another/sdk'},
                           self.fake_run, self.prepared)

    def test_custom_sdk_does_not_require_discovery_of_an_installed_sdk(self):
        def absent(arguments, **kwargs):
            raise FileNotFoundError('xcrun')
        for arguments, environment in [([str(self.archive)], {'SDKROOT': '/custom/sdk'}),
                                       ([str(self.archive), '-isysroot/custom/sdk'], {})]:
            command, _ = linker.command(arguments, environment, absent, self.prepared)
            self.assertEqual(command[-2], '/custom/sdk')

    def test_shell_fast_path_needs_no_python_or_llvm(self):
        tools = self.root / 'bin'
        tools.mkdir()
        cc = tools / 'cc'
        cc.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\nexit 17\n')
        cc.chmod(0o755)
        wrapper = Path(__file__).with_name('rust-linker-macos.sh').resolve()
        for arguments in (['ordinary object.o', '-o', 'result'],
                          ['object.o', '-o', 'libLLVM-output.a'],
                          [str(self.archive), '-fuse-ld=/explicit/linker']):
            result = subprocess.run([str(wrapper), *arguments], env={'PATH': str(tools)},
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 17)
            self.assertEqual(result.stdout.splitlines(), arguments)

    def test_shell_inspects_response_filenames_that_begin_with_a_dash(self):
        library = self.root / 'libLLVMCore.a'
        library.write_bytes(self.archive.read_bytes())
        response = self.root / '-n'
        response.write_text(shlex.join(['-L', str(self.root), '-lLLVMCore']))
        environment = dict(os.environ, DARTFORGE_LLVM_DIR=str(self.root / 'missing LLVM'))
        result = subprocess.run([str(Path(__file__).with_name('rust-linker-macos.sh').resolve()), '@-n'],
                                cwd=self.root, env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('Required executable is missing', result.stderr)

    def test_shell_inspects_nested_response_files(self):
        library = self.root / 'libLLVMCore.a'
        library.write_bytes(self.archive.read_bytes())
        inner = self.root / 'inner arguments.rsp'
        inner.write_text(shlex.join(['-L', str(self.root), '-lLLVMCore']))
        outer = self.root / 'outer.rsp'
        outer.write_text(shlex.join(['@' + str(inner)]))
        environment = dict(os.environ, DARTFORGE_LLVM_DIR=str(self.root / 'missing LLVM'))
        result = subprocess.run([str(Path(__file__).with_name('rust-linker-macos.sh').resolve()),
                                 '@' + str(outer)], env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn('Required executable is missing', result.stderr)


if __name__ == '__main__':
    unittest.main()
