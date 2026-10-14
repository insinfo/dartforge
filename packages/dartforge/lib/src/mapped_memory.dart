import 'dart:collection';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'scoped_resource.dart';

/// An explicitly owned shared file mapping on 64-bit macOS, Linux or Windows.
///
/// ```dart
/// import 'package:dartforge/memory.dart';
/// void main() {
///   final mapping = MappedMemory.open('data.bin');
///   try {
///     print(mapping.bytes.take(16).toList());
///   } finally {
///     mapping.close();
///   }
/// }
/// ```
/// Views perform zero-copy byte access, but are checked fixed-length [List]s,
/// not raw typed lists or pointers. Copies made with `toList` or `sublist` own
/// their bytes. Views cannot be accessed after close or from another isolate.
/// There is no automatic disposal: use `try/finally` or `using`.
///
/// The caller must prevent external truncation while mapped. OS mapping faults
/// caused by truncation or storage failures cannot be converted to Dart errors.
/// Concurrent writers require external synchronization; flush is not a transaction.
final class MappedMemory implements Finalizable, ScopedResource {
  MappedMemory._(this.path, this.length, this.writable, this._native)
      : _owner = Isolate.current.controlPort;

  /// Whether the portable FFI implementation supports this host.
  static bool get isSupported =>
      sizeOf<IntPtr>() == 8 &&
      (Platform.isMacOS || Platform.isLinux || Platform.isWindows);

  /// Maps [length] bytes starting at [offset], or the remainder of the file.
  ///
  /// The file must already exist. Writable mappings update the existing file;
  /// opening a mapping never grows it. Empty ranges are valid.
  ///
  /// # Errors
  /// Throws [UnsupportedError] on unsupported hosts, [ArgumentError] for invalid
  /// paths, [RangeError] for invalid ranges, or [FileSystemException] for OS errors.
  static MappedMemory open(String path,
      {int offset = 0, int? length, bool writable = false}) {
    if (!isSupported) throw UnsupportedError('File mapping is unsupported');
    if (path.isEmpty || path.contains('\u0000')) {
      throw ArgumentError.value(
          path, 'path', 'Expected a nonempty path without NUL');
    }
    if (offset < 0) throw RangeError.value(offset, 'offset');
    if (length != null && length < 0) throw RangeError.value(length, 'length');
    final native = _NativeMapping.open(path, offset, length, writable);
    return MappedMemory._(path, native.length, writable, native);
  }

  final String path;
  final int length;
  final bool writable;
  final SendPort _owner;
  final _NativeMapping _native;
  bool _closed = false;
  int _borrows = 0;
  bool get isClosed => _closed;

  void _checkOwner() {
    if (_owner != Isolate.current.controlPort) {
      throw StateError('A mapping can only be used by its owning isolate');
    }
  }

  void _checkOpen() {
    _checkOwner();
    if (_closed) throw StateError('The mapping is closed');
  }

  /// Returns a revocable view of the full mapped range.
  MappedBytes get bytes => view();

  /// Returns a checked view; byte offsets are relative to this mapping.
  ///
  /// # Errors
  /// Throws [StateError] after close or outside the owner, and [RangeError] for
  /// a range outside the mapping.
  MappedBytes view({int offset = 0, int? length}) {
    _checkOpen();
    final count = _range(this.length, offset, length);
    return MappedBytes._(this, offset, count);
  }

  /// Borrows an external typed view for the synchronous duration of [body].
  ///
  /// The body MUST NOT retain/return the view, derived views, its buffer or a
  /// Future, and MUST finish synchronously. These restrictions are a caller
  /// contract, not an escape proof. Escaped views can access unmapped memory
  /// after close. Read-only mappings provide an unmodifiable typed view.
  /// Use [bytes] when revocable lifetime checks are required.
  ///
  /// # Errors
  /// Throws [StateError] outside the open owner lifetime. Body errors propagate;
  /// closing while a borrow is active throws [StateError].
  T withBytes<T>(T Function(Uint8List bytes) body) {
    _checkOpen();
    // An empty mapping has no address to expose.
    final raw = length == 0 ? Uint8List(0) : _native.data.asTypedList(length);
    final borrowed = writable ? raw : raw.asUnmodifiableView();
    _borrows++;
    try {
      return body(borrowed);
    } finally {
      _borrows--;
    }
  }

  /// Requests synchronous OS writeback for a writable mapping and its file.
  ///
  /// # Errors
  /// Throws [StateError] for an invalid lifetime/owner, [UnsupportedError] for
  /// read-only mappings, and [FileSystemException] when writeback fails.
  void flush() {
    _checkOpen();
    if (!writable) throw UnsupportedError('Cannot flush a read-only mapping');
    _native.flush();
  }

  /// Invalidates all views and releases the mapping and OS handles.
  ///
  /// Idempotent on the owner. If unmapping fails the mapping remains open and
  /// may be retried. Once unmapped, views stay closed even if handle cleanup
  /// reports an error. POSIX descriptor close is never retried after an error.
  ///
  /// # Errors
  /// Throws [StateError] outside the owner, or [FileSystemException] on OS failure.
  void close() {
    _checkOwner();
    if (_closed) return;
    if (_borrows != 0)
      throw StateError('Cannot close during an active byte borrow');
    _native.unmap();
    _closed = true;
    _native.closeHandles();
  }

  @override
  void dispose() => close();
}

/// A fixed-length, revocable byte view. Every access checks its owner's lifetime.
///
/// Writes store the low eight bits, like a Uint8List. Read-only writes throw
/// [UnsupportedError]; closed views throw [StateError], including iterators.
/// List operations that copy bytes return independent Dart lists. Operations
/// can be partially completed if a user callback closes the mapping mid-operation.
final class MappedBytes extends ListBase<int> {
  MappedBytes._(this._mapping, this._offset, this._length);
  final MappedMemory _mapping;
  final int _offset;
  final int _length;

  @override
  int get length {
    _mapping._checkOpen();
    return _length;
  }

  @override
  set length(int value) {
    _mapping._checkOpen();
    throw UnsupportedError('Mapped byte views have fixed length');
  }

  @override
  int operator [](int index) {
    _mapping._checkOpen();
    RangeError.checkValidIndex(index, this, 'index', _length);
    return _mapping._native.data[_offset + index];
  }

  @override
  void operator []=(int index, int value) {
    _mapping._checkOpen();
    RangeError.checkValidIndex(index, this, 'index', _length);
    if (!_mapping.writable) throw UnsupportedError('The mapping is read-only');
    _mapping._native.data[_offset + index] = value & 255;
  }
}

int _range(int total, int offset, int? length) {
  if (offset < 0 || offset > total) throw RangeError.value(offset, 'offset');
  final count = length ?? total - offset;
  if (count < 0 || count > total - offset)
    throw RangeError.value(count, 'length');
  return count;
}

// All ABI layouts below are restricted to 64-bit supported hosts.
final class _NativeMapping {
  _NativeMapping(this.path, this.file, this.mapping, this.base,
      this.mappedLength, this.delta, this.length, this.windows);
  final String path;
  final int file;
  final Pointer<Void> mapping;
  final Pointer<Void> base;
  final int mappedLength;
  final int delta;
  final int length;
  final bool windows;
  Pointer<Uint8> get data => base.cast<Uint8>() + delta;

  static final DynamicLibrary _lib = Platform.isWindows
      ? DynamicLibrary.open('kernel32.dll')
      : DynamicLibrary.process();
  static final DynamicLibrary _allocator = Platform.isWindows
      ? DynamicLibrary.open('ucrtbase.dll')
      : DynamicLibrary.process();
  static final _calloc = _allocator.lookupFunction<
      Pointer<Void> Function(Size, Size),
      Pointer<Void> Function(int, int)>('calloc');
  static final _free = _allocator.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('free');
  static Pointer<Void> _allocate(int bytes) {
    final result = _calloc(1, bytes);
    if (result.address == 0) throw StateError('Native allocation failed');
    return result;
  }

  static final _lastWindowsError =
      _lib.lookupFunction<Uint32 Function(), int Function()>('GetLastError');
  static final _errno =
      _lib.lookupFunction<Pointer<Int32> Function(), Pointer<Int32> Function()>(
          Platform.isMacOS ? '__error' : '__errno_location');

  static FileSystemException _error(String path, String operation) {
    // Resolve error accessors before an OS operation, so dlsym/GetProcAddress
    // cannot overwrite the failing operation's error code.
    final code = Platform.isWindows ? _lastWindowsError() : _errno().value;
    return FileSystemException(
        '$operation failed', path, OSError('OS error', code));
  }

  static _NativeMapping open(
      String path, int offset, int? count, bool writable) {
    if (Platform.isWindows) {
      _lastWindowsError();
    } else {
      _errno();
    }
    return Platform.isWindows
        ? _openWindows(path, offset, count, writable)
        : _openPosix(path, offset, count, writable);
  }

  static _NativeMapping _openPosix(
      String path, int offset, int? count, bool writable) {
    final encoded = utf8.encode(path);
    final name = _allocate(encoded.length + 1).cast<Uint8>();
    int fd;
    try {
      for (var i = 0; i < encoded.length; i++) {
        name[i] = encoded[i];
      }
      fd = _lib.lookupFunction<Int32 Function(Pointer<Uint8>, Int32),
          int Function(Pointer<Uint8>, int)>('open')(name, writable ? 2 : 0);
      if (fd < 0) throw _error(path, 'open');
    } finally {
      _free(name.cast<Void>());
    }
    try {
      final size = _lib.lookupFunction<Int64 Function(Int32, Int64, Int32),
          int Function(int, int, int)>('lseek')(fd, 0, 2);
      if (size < 0) throw _error(path, 'lseek');
      final length = _range(size, offset, count);
      final page = _lib
          .lookupFunction<Int32 Function(), int Function()>('getpagesize')();
      if (page <= 0) throw StateError('Invalid OS page size');
      final aligned = offset - offset % page;
      final delta = offset - aligned;
      final bytes = length == 0 ? 0 : length + delta;
      final base = bytes == 0
          ? nullptr
          : _lib.lookupFunction<
                  Pointer<Void> Function(
                      Pointer<Void>, Size, Int32, Int32, Int32, Int64),
                  Pointer<Void> Function(
                      Pointer<Void>, int, int, int, int, int)>('mmap')(
              nullptr, bytes, writable ? 3 : 1, 1, fd, aligned);
      if (base.address == -1) throw _error(path, 'mmap');
      return _NativeMapping(
          path, fd, nullptr, base, bytes, delta, length, false);
    } catch (_) {
      _lib.lookupFunction<Int32 Function(Int32), int Function(int)>('close')(
          fd);
      rethrow;
    }
  }

  static _NativeMapping _openWindows(
      String path, int offset, int? count, bool writable) {
    final units = path.codeUnits;
    final name = _allocate((units.length + 1) * 2).cast<Uint16>();
    Pointer<Void> file;
    try {
      for (var i = 0; i < units.length; i++) {
        name[i] = units[i];
      }
      file =
          _lib.lookupFunction<
                  Pointer<Void> Function(Pointer<Uint16>, Uint32, Uint32,
                      Pointer<Void>, Uint32, Uint32, Pointer<Void>),
                  Pointer<Void> Function(Pointer<Uint16>, int, int,
                      Pointer<Void>, int, int, Pointer<Void>)>('CreateFileW')(
              name,
              writable ? 0xc0000000 : 0x80000000,
              7,
              nullptr,
              3,
              128,
              nullptr);
      if (file.address == -1) throw _error(path, 'CreateFileW');
    } finally {
      _free(name.cast<Void>());
    }
    Pointer<Void> mapping = nullptr;
    try {
      final info = _allocate(48);
      int size;
      int page;
      try {
        if (_lib.lookupFunction<Int32 Function(Pointer<Void>, Pointer<Int64>),
                    int Function(Pointer<Void>, Pointer<Int64>)>(
                'GetFileSizeEx')(file, info.cast<Int64>()) ==
            0) throw _error(path, 'GetFileSizeEx');
        size = info.cast<Int64>().value;
        _lib.lookupFunction<Void Function(Pointer<Void>),
            void Function(Pointer<Void>)>('GetSystemInfo')(info);
        page = (info.cast<Uint8>() + 40).cast<Uint32>().value;
      } finally {
        _free(info);
      }
      final length = _range(size, offset, count);
      if (page <= 0) throw StateError('Invalid OS allocation granularity');
      final aligned = offset - offset % page;
      final delta = offset - aligned;
      final bytes = length == 0 ? 0 : length + delta;
      Pointer<Void> base = nullptr;
      if (bytes != 0) {
        mapping =
            _lib.lookupFunction<
                    Pointer<Void> Function(Pointer<Void>, Pointer<Void>, Uint32,
                        Uint32, Uint32, Pointer<Uint16>),
                    Pointer<Void> Function(Pointer<Void>, Pointer<Void>, int,
                        int, int, Pointer<Uint16>)>('CreateFileMappingW')(
                file, nullptr, writable ? 4 : 2, 0, 0, nullptr);
        if (mapping.address == 0) throw _error(path, 'CreateFileMappingW');
        base =
            _lib.lookupFunction<
                    Pointer<Void> Function(
                        Pointer<Void>, Uint32, Uint32, Uint32, Size),
                    Pointer<Void> Function(
                        Pointer<Void>, int, int, int, int)>('MapViewOfFile')(
                mapping,
                writable ? 2 : 4,
                (aligned >> 32) & 0xffffffff,
                aligned & 0xffffffff,
                bytes);
        if (base.address == 0) throw _error(path, 'MapViewOfFile');
      }
      return _NativeMapping(
          path, file.address, mapping, base, bytes, delta, length, true);
    } catch (_) {
      if (mapping.address != 0) _closeWindows(mapping);
      _closeWindows(file);
      rethrow;
    }
  }

  static int _closeWindows(Pointer<Void> handle) => _lib.lookupFunction<
      Int32 Function(Pointer<Void>),
      int Function(Pointer<Void>)>('CloseHandle')(handle);

  void flush() {
    if (mappedLength == 0) return;
    if (windows) {
      if (_lib.lookupFunction<
              Int32 Function(Pointer<Void>, Size),
              int Function(
                  Pointer<Void>, int)>('FlushViewOfFile')(base, mappedLength) ==
          0) {
        throw _error(path, 'FlushViewOfFile');
      }
      if (_lib.lookupFunction<Int32 Function(Pointer<Void>),
                  int Function(Pointer<Void>)>('FlushFileBuffers')(
              Pointer<Void>.fromAddress(file)) ==
          0) {
        throw _error(path, 'FlushFileBuffers');
      }
    } else {
      if (_lib.lookupFunction<Int32 Function(Pointer<Void>, Size, Int32),
                  int Function(Pointer<Void>, int, int)>('msync')(
              base, mappedLength, Platform.isMacOS ? 0x10 : 4) !=
          0) {
        throw _error(path, 'msync');
      }
      if (_lib.lookupFunction<Int32 Function(Int32), int Function(int)>(
              'fsync')(file) !=
          0) {
        throw _error(path, 'fsync');
      }
    }
  }

  void unmap() {
    if (mappedLength == 0) return;
    final result = windows
        ? _lib.lookupFunction<Int32 Function(Pointer<Void>),
            int Function(Pointer<Void>)>('UnmapViewOfFile')(base)
        : _lib.lookupFunction<Int32 Function(Pointer<Void>, Size),
            int Function(Pointer<Void>, int)>('munmap')(base, mappedLength);
    if (windows ? result == 0 : result != 0) throw _error(path, 'unmap');
  }

  void closeHandles() {
    if (windows) {
      FileSystemException? failure;
      if (mapping.address != 0 && _closeWindows(mapping) == 0) {
        failure = _error(path, 'CloseHandle(mapping)');
      }
      if (_closeWindows(Pointer<Void>.fromAddress(file)) == 0) {
        failure ??= _error(path, 'CloseHandle(file)');
      }
      if (failure != null) throw failure;
    } else {
      if (_lib.lookupFunction<Int32 Function(Int32), int Function(int)>(
              'close')(file) !=
          0) {
        throw _error(path, 'close');
      }
    }
  }
}
