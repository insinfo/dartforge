// Copyright (c) 2012, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

// O `dart:mirrors` do backend nativo do DartForge (B01): o subconjunto que os
// builders do ecossistema usam. O `TypeChecker.fromRuntime(T)` do
// `source_gen` chama `reflectClass(T)` e lê só o nome da classe
// (`MirrorSystem.getName(mirror.simpleName)`) e a URI da biblioteca que a
// declara (`(mirror.owner as LibraryMirror).uri`). O resto da reflexão não
// existe no nativo — como no AOT da VM — e lança `UnsupportedError`.
// Substitui `_internal/vm/lib/mirrors_patch.dart` (e as partes dele, que
// dependem dos natives de reflexão da VM).

import "dart:_internal" as internal;

import "dart:_internal" show patch;

Never _semReflexao(String o) =>
    throw new UnsupportedError("dart:mirrors: $o não existe no DartForge nativo "
        "(só reflectClass/reflect com o nome e a biblioteca da classe)");

@patch
MirrorSystem currentMirrorSystem() => _semReflexao("currentMirrorSystem");

@patch
InstanceMirror reflect(dynamic reflectee) => new _InstanceMirrorDf(reflectee);

@patch
ClassMirror reflectClass(Type key) {
  final nome = _nome(key);
  final uri = _uri(key);
  if (nome == null || uri == null) {
    throw new ArgumentError("$key does not denote a class");
  }
  return new _ClassMirrorDf(key, nome, Uri.parse(uri));
}

@patch
TypeMirror reflectType(Type key, [List<Type>? typeArguments]) {
  if (typeArguments != null) _semReflexao("reflectType com argumentos de tipo");
  return reflectClass(key);
}

@pragma("vm:external-name", "DartForge_mirrors_nome")
external String? _nome(Type tipo);

@pragma("vm:external-name", "DartForge_mirrors_uri")
external String? _uri(Type tipo);

@patch
class MirrorSystem {
  @patch
  LibraryMirror findLibrary(Symbol libraryName) =>
      _semReflexao("MirrorSystem.findLibrary");

  @patch
  static String getName(Symbol symbol) {
    return internal.Symbol.computeUnmangledName(symbol as internal.Symbol);
  }

  @patch
  static Symbol getSymbol(String name, [LibraryMirror? library]) {
    if (library != null || (name.length > 0 && name[0] == '_')) {
      _semReflexao("MirrorSystem.getSymbol de nome privado");
    }
    return new internal.Symbol.unvalidated(name);
  }
}

@patch
class AbstractClassInstantiationError {
  @patch
  String toString() => "Cannot instantiate abstract class $_className";
}

/// A biblioteca que declara uma classe: só a URI e o nome simples.
final class _LibraryMirrorDf implements LibraryMirror {
  final Uri uri;
  _LibraryMirrorDf(this.uri);

  Symbol get simpleName => new internal.Symbol.unvalidated("");
  Symbol get qualifiedName => simpleName;
  DeclarationMirror? get owner => null;
  bool get isPrivate => false;
  bool get isTopLevel => false;

  bool operator ==(Object other) =>
      other is _LibraryMirrorDf && other.uri == uri;
  int get hashCode => uri.hashCode;
  String toString() => "LibraryMirror on '$uri'";

  noSuchMethod(Invocation i) =>
      _semReflexao("LibraryMirror.${MirrorSystem.getName(i.memberName)}");
}

/// Uma classe pelo objeto `Type`: nome declarado e biblioteca.
final class _ClassMirrorDf implements ClassMirror {
  final Type _tipo;
  final String _nomeDaClasse;
  final Uri _uriDaBiblioteca;
  _ClassMirrorDf(this._tipo, this._nomeDaClasse, this._uriDaBiblioteca);

  Symbol get simpleName => new internal.Symbol.unvalidated(_nomeDaClasse);
  Symbol get qualifiedName => new internal.Symbol.unvalidated(
      "${_uriDaBiblioteca}.$_nomeDaClasse");
  DeclarationMirror? get owner => new _LibraryMirrorDf(_uriDaBiblioteca);
  bool get isPrivate => _nomeDaClasse.startsWith("_");
  bool get isTopLevel => true;
  bool get hasReflectedType => true;
  Type get reflectedType => _tipo;

  bool operator ==(Object other) =>
      other is _ClassMirrorDf && other._tipo == _tipo;
  int get hashCode => _tipo.hashCode;
  String toString() => "ClassMirror on '$_nomeDaClasse'";

  noSuchMethod(Invocation i) =>
      _semReflexao("ClassMirror.${MirrorSystem.getName(i.memberName)}");
}

/// Um objeto: o próprio e a classe do tipo em tempo de execução.
final class _InstanceMirrorDf implements InstanceMirror {
  final Object? reflectee;
  _InstanceMirrorDf(this.reflectee);

  bool get hasReflectee => true;
  ClassMirror get type => reflectClass(reflectee.runtimeType);

  bool operator ==(Object other) =>
      other is _InstanceMirrorDf && identical(other.reflectee, reflectee);
  int get hashCode => identityHashCode(reflectee);
  String toString() => "InstanceMirror on ${Error.safeToString(reflectee)}";

  noSuchMethod(Invocation i) =>
      _semReflexao("InstanceMirror.${MirrorSystem.getName(i.memberName)}");
}
