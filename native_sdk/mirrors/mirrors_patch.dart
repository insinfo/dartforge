// Copyright (c) 2012, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

// DartForge native backend's `dart:mirrors` (B01): the subset the ecosystem's
// builders use. `source_gen`'s `TypeChecker.fromRuntime(T)` calls
// `reflectClass(T)` and only reads the class name
// (`MirrorSystem.getName(mirror.simpleName)`) and the URI of the library that
// declares it (`(mirror.owner as LibraryMirror).uri`). The rest of reflection
// does not exist natively — as in the VM's AOT — and throws `UnsupportedError`.
// Replaces `_internal/vm/lib/mirrors_patch.dart` (and its parts, which depend
// on the VM's reflection natives).

import "dart:_internal" as internal;

import "dart:_internal" show patch;

Never _noReflection(String o) =>
    throw new UnsupportedError("dart:mirrors: $o does not exist in native DartForge "
        "(only reflectClass/reflect with the class name and library)");

@patch
MirrorSystem currentMirrorSystem() => _noReflection("currentMirrorSystem");

@patch
InstanceMirror reflect(dynamic reflectee) => new _InstanceMirrorDf(reflectee);

@patch
ClassMirror reflectClass(Type key) {
  final nameArg = _nameNative(key);
  final uri = _uri(key);
  if (nameArg == null || uri == null) {
    throw new ArgumentError("$key does not denote a class");
  }
  return new _ClassMirrorDf(key, nameArg, Uri.parse(uri));
}

@patch
TypeMirror reflectType(Type key, [List<Type>? typeArguments]) {
  if (typeArguments != null) _noReflection("reflectType with type arguments");
  return reflectClass(key);
}

@pragma("vm:external-name", "DartForge_mirrors_name")
external String? _nameNative(Type typeArg);

@pragma("vm:external-name", "DartForge_mirrors_uri")
external String? _uri(Type typeArg);

@patch
class MirrorSystem {
  @patch
  LibraryMirror findLibrary(Symbol libraryName) =>
      _noReflection("MirrorSystem.findLibrary");

  @patch
  static String getName(Symbol symbol) {
    return internal.Symbol.computeUnmangledName(symbol as internal.Symbol);
  }

  @patch
  static Symbol getSymbol(String name, [LibraryMirror? library]) {
    if (library != null || (name.length > 0 && name[0] == '_')) {
      _noReflection("MirrorSystem.getSymbol of a private name");
    }
    return new internal.Symbol.unvalidated(name);
  }
}

@patch
class AbstractClassInstantiationError {
  @patch
  String toString() => "Cannot instantiate abstract class $_className";
}

/// The library that declares a class: only the URI and the simple name.
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
      _noReflection("LibraryMirror.${MirrorSystem.getName(i.memberName)}");
}

/// A class through the `Type` object: the declared name and the library.
final class _ClassMirrorDf implements ClassMirror {
  final Type _typeNative;
  final String _classNameOf;
  final Uri _libraryUri;
  _ClassMirrorDf(this._typeNative, this._classNameOf, this._libraryUri);

  Symbol get simpleName => new internal.Symbol.unvalidated(_classNameOf);
  Symbol get qualifiedName => new internal.Symbol.unvalidated(
      "${_libraryUri}.$_classNameOf");
  DeclarationMirror? get owner => new _LibraryMirrorDf(_libraryUri);
  bool get isPrivate => _classNameOf.startsWith("_");
  bool get isTopLevel => true;
  bool get hasReflectedType => true;
  Type get reflectedType => _typeNative;

  bool operator ==(Object other) =>
      other is _ClassMirrorDf && other._typeNative == _typeNative;
  int get hashCode => _typeNative.hashCode;
  String toString() => "ClassMirror on '$_classNameOf'";

  noSuchMethod(Invocation i) =>
      _noReflection("ClassMirror.${MirrorSystem.getName(i.memberName)}");
}

/// An object: itself and the class of its runtime type.
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
      _noReflection("InstanceMirror.${MirrorSystem.getName(i.memberName)}");
}
