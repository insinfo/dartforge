import 'dart:ffi';

import 'scoped_resource.dart';

/// One failed candidate in [NativeLibrary.openFirst].
final class NativeLibraryLoadFailure {
  const NativeLibraryLoadFailure(this.path, this.error);

  final String path;
  final ArgumentError error;

  @override
  String toString() => '$path: $error';
}

/// All candidates failed to load, in their original order.
final class NativeLibraryOpenError extends Error {
  NativeLibraryOpenError(Iterable<NativeLibraryLoadFailure> failures)
      : failures = List.unmodifiable(failures);

  final List<NativeLibraryLoadFailure> failures;

  @override
  String toString() => 'NativeLibraryOpenError: ${failures.join('; ')}';
}

/// An owned dynamic library selected from ordered platform candidates.
///
/// Uses the official FFI API on both DartForge and the Dart VM. Loading and
/// symbol resolution do not change the library's ABI or make calls thread-safe.
/// Keep the library open until all native calls and callbacks using it finish.
///
/// ```dart
/// final library = NativeLibrary.openFirst(['/usr/lib/libSystem.B.dylib']);
/// try {
///   print(library.hasSymbol('strlen'));
/// } finally {
///   library.close();
/// }
/// ```
final class NativeLibrary implements ScopedResource {
  NativeLibrary._(this._library, this.selectedPath);

  final DynamicLibrary _library;
  final String selectedPath;
  bool _closed = false;

  /// Available wherever the Dart native FFI library is supported.
  static bool get isSupported => true;

  /// Opens the first loadable candidate, without attempting later candidates.
  ///
  /// Throws [ArgumentError] for an empty list or an empty/NUL-containing path.
  /// Throws [NativeLibraryOpenError] if every candidate fails; its [failures]
  /// preserve each path and the original loader error. Other failures propagate.
  static NativeLibrary openFirst(Iterable<String> candidates) {
    final paths = candidates.toList(growable: false);
    if (paths.isEmpty) {
      throw ArgumentError.value(paths, 'candidates', 'Must not be empty');
    }
    for (final path in paths) {
      _validateName(path, 'candidates');
    }
    final failures = <NativeLibraryLoadFailure>[];
    for (final path in paths) {
      try {
        return NativeLibrary._(DynamicLibrary.open(path), path);
      } on ArgumentError catch (error) {
        failures.add(NativeLibraryLoadFailure(path, error));
      }
    }
    throw NativeLibraryOpenError(failures);
  }

  /// Whether this owner has been closed, including a failed close attempt.
  bool get isClosed => _closed;

  /// Borrows the SDK handle for FFI operations such as `lookupFunction`.
  ///
  /// Throws [StateError] after close. Do not close this borrowed handle; close
  /// the owner instead. Returned function/pointer values require the owner to
  /// remain open and become unusable after close.
  DynamicLibrary get library {
    _checkOpen();
    return _library;
  }

  /// Checks for a symbol without turning a missing symbol into an exception.
  ///
  /// Throws [StateError] after close, or [ArgumentError] for an empty/NUL name.
  bool hasSymbol(String name) {
    _checkOpen();
    _validateName(name, 'name');
    return _library.providesSymbol(name);
  }

  /// Looks up a pointer whose validity is bounded by this owner's lifetime.
  ///
  /// Throws [StateError] after close, or [ArgumentError] for an invalid/missing
  /// symbol. This does not validate the symbol's native type.
  Pointer<T> lookup<T extends NativeType>(String name) {
    _checkOpen();
    _validateName(name, 'name');
    return _library.lookup<T>(name);
  }

  /// Releases the owned loader reference once; repeated calls do nothing.
  ///
  /// Existing pointers and functions must no longer be used. If the SDK close
  /// operation throws, the owner remains closed so it cannot unload twice.
  void close() {
    if (_closed) return;
    _closed = true;
    _library.close();
  }

  /// Releases the owned library for a [ScopedResource] scope.
  @override
  void dispose() => close();

  void _checkOpen() {
    if (_closed) throw StateError('The native library is closed');
  }

  static void _validateName(String value, String argument) {
    if (value.isEmpty || value.contains('\u0000')) {
      throw ArgumentError.value(
          value, argument, 'Must be nonempty without NUL');
    }
  }
}
