"""Check SDK selection, explicit configuration and lossless rustc flags."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'cargo_macos_llvm', Path(__file__).with_name('cargo-macos-llvm.py'))
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


class MacosLinkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.prefix = self.root / 'LLVM with spaces'
        (self.prefix / 'bin').mkdir(parents=True)
        for name in ('clang', 'ld64.lld'):
            tool = self.prefix / 'bin' / name
            tool.touch()
            tool.chmod(0o755)
        self.new_sdk = self.root / 'MacOSX27.sdk'
        self.old_sdk = self.root / 'MacOSX26.sdk'
        self.new_sdk.mkdir()
        self.old_sdk.mkdir()
        self.calls = []

    def fake_run(self, args, **kwargs):
        self.calls.append((args, kwargs))
        if args[0] == 'xcrun':
            return subprocess.CompletedProcess(args, 0, str(self.new_sdk), '')
        compatible = args[args.index('-isysroot') + 1] == str(self.old_sdk)
        self.assertEqual(kwargs['input'], 'int main(void) { return 0; }\n')
        return subprocess.CompletedProcess(args, 0 if compatible else 1, '', 'bad stub')

    def test_probes_sdk_and_preserves_paths_and_flags(self):
        original = {'DARTFORGE_LLVM_DIR': str(self.prefix),
                    'RUSTFLAGS': '-C "metadata=contains spaces"'}
        prepared = helper.prepare(original, self.fake_run)
        self.assertEqual(prepared['SDKROOT'], str(self.old_sdk))
        self.assertEqual(prepared['CARGO_ENCODED_RUSTFLAGS'].split('\x1f'),
                         ['-C', 'metadata=contains spaces', '-C',
                          f'link-arg=-fuse-ld={self.prefix}/bin/ld64.lld'])
        self.assertNotIn('SDKROOT', original)
        self.assertEqual(len(self.calls), 3)

    def test_explicit_incompatible_sdk_is_not_replaced(self):
        with self.assertRaisesRegex(ValueError, 'No compatible macOS SDK'):
            helper.prepare({'DARTFORGE_LLVM_DIR': str(self.prefix),
                            'SDKROOT': str(self.new_sdk)}, self.fake_run)
        self.assertEqual(len(self.calls), 1)

    def test_encoded_flags_take_precedence_even_when_empty(self):
        for flags in ('', '-C\x1fdebuginfo=1'):
            with self.subTest(flags=flags):
                prepared = helper.prepare(
                    {'LLVM_SYS_221_PREFIX': str(self.prefix),
                     'SDKROOT': str(self.old_sdk), 'RUSTFLAGS': 'ignored',
                     'CARGO_ENCODED_RUSTFLAGS': flags}, self.fake_run)
                expected = (flags + '\x1f') if flags else ''
                self.assertTrue(prepared['CARGO_ENCODED_RUSTFLAGS'].startswith(expected + '-C\x1f'))
                self.assertNotIn('ignored', prepared['CARGO_ENCODED_RUSTFLAGS'])

    def test_missing_tool_fails_before_running_cargo(self):
        (self.prefix / 'bin/ld64.lld').unlink()
        with self.assertRaisesRegex(ValueError, 'Required executable is missing'):
            helper.prepare({'DARTFORGE_LLVM_DIR': str(self.prefix)}, self.fake_run)
        self.assertEqual(self.calls, [])

    def test_path_discovery_skips_wrong_versions_and_uses_reported_prefix(self):
        def probe(arguments, **kwargs):
            if arguments == ['llvm-config-22', '--version']:
                return subprocess.CompletedProcess(arguments, 0, '21.1.8\n', '')
            if arguments == ['llvm-config', '--version']:
                return subprocess.CompletedProcess(arguments, 0, '22.1.8\n', '')
            if arguments == ['llvm-config', '--prefix']:
                return subprocess.CompletedProcess(arguments, 0, str(self.prefix) + '\n', '')
            raise FileNotFoundError(arguments[0])
        self.assertEqual(helper.llvm_prefix({}, probe), self.prefix)

    def test_missing_path_tool_reports_configuration_and_explicit_prefix_wins(self):
        def missing(arguments, **kwargs):
            raise FileNotFoundError(arguments[0])
        with self.assertRaisesRegex(ValueError, 'LLVM 22.1 was not found'):
            helper.llvm_prefix({}, missing)
        self.assertEqual(helper.llvm_prefix({'DARTFORGE_LLVM_DIR': str(self.prefix),
                                            'LLVM_SYS_221_PREFIX': '/unused'}, missing), self.prefix)

    def test_project_sdk_pin_is_shared_with_the_cargo_helper(self):
        configured = {'DARTFORGE_LLVM_DIR': str(self.prefix),
                      'DARTFORGE_MACOS_SDK_ROOT': str(self.old_sdk)}
        self.assertEqual(helper.prepare(configured, self.fake_run)['SDKROOT'], str(self.old_sdk))
        configured['SDKROOT'] = str(self.new_sdk)
        with self.assertRaisesRegex(ValueError, 'conflicts with SDKROOT'):
            helper.prepare(configured, self.fake_run)


if __name__ == '__main__':
    unittest.main()
