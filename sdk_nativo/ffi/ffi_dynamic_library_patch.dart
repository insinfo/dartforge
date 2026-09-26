// Patch do `dart:ffi` para o backend nativo do DartForge: a
// `DynamicLibrary` guarda o handle do sistema (`dlopen`/`LoadLibraryW`,
// `crates/runtime/src/ffi.rs`); `lookupFunction` é `lookup` + `asFunction`.

// Copyright (c) 2019, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

import "dart:_internal" show patch;
import 'dart:typed_data';
import 'dart:isolate';
import 'dart:typed_data';

@pragma("vm:external-name", "Ffi_dl_open")
external int _open(String path);
@pragma("vm:external-name", "Ffi_dl_processLibrary")
external int _processLibrary();
@pragma("vm:external-name", "Ffi_dl_executableLibrary")
external int _executableLibrary();
@pragma("vm:external-name", "Ffi_dl_lookup")
external int _lookup(int handle, String symbolName);
@pragma("vm:external-name", "Ffi_dl_providesSymbol")
external bool _providesSymbol(int handle, String symbolName);
@pragma("vm:external-name", "Ffi_dl_close")
external void _close(int handle);

@patch
@pragma("vm:entry-point")
final class DynamicLibrary {
  final int _handle;

  DynamicLibrary._comHandle(this._handle);

  @patch
  factory DynamicLibrary.open(String path) {
    return new DynamicLibrary._comHandle(_open(path));
  }

  @patch
  factory DynamicLibrary.process() =>
      new DynamicLibrary._comHandle(_processLibrary());

  @patch
  factory DynamicLibrary.executable() =>
      new DynamicLibrary._comHandle(_executableLibrary());

  @patch
  Pointer<T> lookup<T extends NativeType>(String symbolName) =>
      new Pointer<T>._comEndereco(_lookup(_handle, symbolName));

  @patch
  bool providesSymbol(String symbolName) => _providesSymbol(_handle, symbolName);

  int getHandle() => _handle;

  @patch
  void close() => _close(_handle);

  @patch
  bool operator ==(Object other) {
    if (other is! DynamicLibrary) return false;
    DynamicLibrary otherLib = other;
    return getHandle() == otherLib.getHandle();
  }

  @patch
  int get hashCode {
    return getHandle().hashCode;
  }

  @patch
  Pointer<Void> get handle => Pointer.fromAddress(getHandle());
}

@patch
extension DynamicLibraryExtension on DynamicLibrary {
  @patch
  DS lookupFunction<NS extends Function, DS extends Function>(String symbolName,
          {bool isLeaf = false}) =>
      lookup<NativeFunction<NS>>(symbolName).asFunction<DS>(isLeaf: isLeaf);
}
