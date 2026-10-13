// Copyright (c) 2013, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

part of "core_patch.dart";

@pragma("vm:entry-point")
final class _Closure implements Function {
  factory _Closure._uninstantiable() {
    throw "Unreachable";
  }

  @pragma("vm:external-name", "Closure_equals")
  external bool operator ==(Object other);

  // DartForge: the hash is computed (consistent with `==`: the function and the
  // tear-off's receiver), without the VM's cache in `_hash`.
  int get hashCode {
    return _computeHash();
  }

  @pragma("vm:entry-point")
  _Closure get call => this;

  @pragma("vm:external-name", "Closure_computeHash")
  external int _computeHash();

  // DartForge (docs/NATIVO-ESPACO-UNIFICADO.md §2.5): the fields in the order of
  // the block the runtime and the generated code create (`_Closure`, cid 11, an
  // `INSTANCE` of four fields). Dart neither reads nor writes them: `==` and
  // `hashCode` need the receiver kept in the `_Contexto`, and stay in the
  // natives. The instance is only born in the runtime/generated code, never
  // through a constructor.
  @pragma("vm:entry-point")
  int _code = 0;
  @pragma("vm:entry-point")
  Object? _context;
  @pragma("vm:entry-point")
  int _typed = 0;
  @pragma("vm:entry-point")
  int _abi = 0;
}
