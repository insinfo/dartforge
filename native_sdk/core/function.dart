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

  // DartForge: o hash é calculado (coerente com `==`: a função e o
  // receptor do tear-off), sem o cache em `_hash` da VM.
  int get hashCode {
    return _computeHash();
  }

  @pragma("vm:entry-point")
  _Closure get call => this;

  @pragma("vm:external-name", "Closure_computeHash")
  external int _computeHash();

  // DartForge (docs/NATIVO-ESPACO-UNIFICADO.md §2.5): os campos na ordem do
  // bloco que o runtime e o código gerado criam (`_Closure`, cid 11,
  // `INSTANCIA` de quatro campos). O Dart não os lê nem grava: `==` e
  // `hashCode` precisam do receptor guardado no `_Contexto`, e ficam nos
  // natives. A instância nasce só no runtime/código gerado, nunca por
  // construtor.
  @pragma("vm:entry-point")
  int _codigo = 0;
  @pragma("vm:entry-point")
  Object? _contexto;
  @pragma("vm:entry-point")
  int _tipado = 0;
  @pragma("vm:entry-point")
  int _abi = 0;
}
