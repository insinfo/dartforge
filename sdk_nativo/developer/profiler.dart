// Copyright (c) 2014, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

// DartForge: o `profiler.dart` da VM com as marcas (`UserTag`) em Dart, sem
// os natives do perfilador (`UserTag_*`, `Profiler_getCurrentTag`). A
// semântica observável é a da VM: a mesma marca para o mesmo rótulo, no
// máximo `UserTag.maxUserTags` marcas, `makeCurrent` devolve a anterior e a
// marca corrente é por isolado (os estáticos são por isolado).

part of "developer.dart";

@patch
class UserTag {
  @patch
  factory UserTag(String label) {
    return new _UserTag(label);
  }
  @patch
  static UserTag get defaultTag => _getDefaultTag();
}

@pragma("vm:entry-point")
final class _UserTag implements UserTag {
  static final Map<String, _UserTag> _marcas = <String, _UserTag>{};
  static final _UserTag _padrao = new _UserTag._('Default');
  static _UserTag? _corrente;

  final String label;

  _UserTag._(this.label);

  factory _UserTag(String label) {
    final existente = _marcas[label];
    if (existente != null) return existente;
    if (_marcas.length >= UserTag.maxUserTags) {
      throw new UnsupportedError(
          'UserTag instance limit (${UserTag.maxUserTags}) reached.');
    }
    return _marcas[label] = new _UserTag._(label);
  }

  UserTag makeCurrent() {
    final anterior = _corrente ?? _padrao;
    _corrente = this;
    return anterior;
  }
}

@patch
UserTag getCurrentTag() => _getCurrentTag();

UserTag _getCurrentTag() => _UserTag._corrente ?? _UserTag._padrao;

UserTag _getDefaultTag() => _UserTag._padrao;
