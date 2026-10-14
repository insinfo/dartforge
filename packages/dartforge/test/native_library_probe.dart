import 'dart:ffi';
import 'dart:io';

import 'package:dartforge/dartforge.dart';

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void rejects<T>(void Function() body, String message) {
  try {
    body();
  } catch (error) {
    if (error is T) return;
    rethrow;
  }
  throw StateError(message);
}

void main() {
  check(NativeLibrary.isSupported, 'Native FFI support missing');
  final candidates = Platform.isMacOS
      ? ['/usr/lib/libSystem.B.dylib']
      : Platform.isWindows
          ? ['ucrtbase.dll', 'msvcrt.dll']
          : ['libc.so.6', 'libc.so'];
  final missing = '${Directory.systemTemp.path}/dartforge-missing-library-';
  rejects<ArgumentError>(
      () => NativeLibrary.openFirst([]), 'Empty list accepted');
  rejects<ArgumentError>(
      () => NativeLibrary.openFirst(['']), 'Empty path accepted');
  rejects<ArgumentError>(
      () => NativeLibrary.openFirst(['bad\u0000path']), 'NUL path accepted');
  try {
    NativeLibrary.openFirst(['${missing}first', '${missing}second']);
    throw StateError('Missing libraries loaded');
  } on NativeLibraryOpenError catch (error) {
    check(error.failures.length == 2, 'Load failures lost');
    check(
        error.failures[0].path == '${missing}first' &&
            error.failures[1].path == '${missing}second',
        'Candidate order changed');
    check(error.toString().contains('${missing}first'), 'Error path lost');
    rejects<UnsupportedError>(() => error.failures.clear(), 'Failures mutable');
  }
  final owner = NativeLibrary.openFirst(['${missing}first', ...candidates]);
  try {
    check(candidates.contains(owner.selectedPath), 'Unexpected selected path');
    check(!owner.isClosed, 'Fresh library closed');
    check(owner.hasSymbol('abs'), 'Existing symbol missing');
    check(!owner.hasSymbol('__dartforge_missing_symbol__'),
        'Missing symbol found');
    check(owner.lookup<Void>('abs').address != 0, 'Null symbol address');
    final abs = owner.library
        .lookupFunction<Int32 Function(Int32), int Function(int)>('abs');
    check(abs(-37) == 37, 'Loaded function returned wrong value');
    rejects<ArgumentError>(() => owner.hasSymbol(''), 'Empty symbol accepted');
    rejects<ArgumentError>(
        () => owner.hasSymbol('abs\u0000tail'), 'NUL symbol accepted');
    rejects<ArgumentError>(
        () => owner.lookup<Void>('__dartforge_missing_symbol__'),
        'Missing lookup succeeded');
  } finally {
    owner.close();
  }
  owner.close();
  check(owner.isClosed, 'Close did not update state');
  rejects<StateError>(
      () => owner.hasSymbol('abs'), 'Closed symbol query accepted');
  rejects<StateError>(
      () => owner.lookup<Void>('abs'), 'Closed lookup accepted');
  rejects<StateError>(() {
    owner.library;
  }, 'Closed borrowed handle accepted');
  final scoped = NativeLibrary.openFirst(candidates);
  check(using(scoped, (resource) => resource.hasSymbol('abs')),
      'Scoped symbol lookup failed');
  check(scoped.isClosed, 'Scoped library was not closed');
  final failedScope = NativeLibrary.openFirst(candidates);
  rejects<StateError>(
      () => using(failedScope, (_) => throw StateError('scope failure')),
      'Scope failure did not propagate');
  check(failedScope.isClosed, 'Failed scope did not close library');
  print('native library fallback, symbols and lifecycle verified');
}
