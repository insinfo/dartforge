// Patch do `RegExp` para o backend nativo do DartForge (sobreposição
// `sdk_nativo/`). É o `_internal/vm/lib/regexp_patch.dart` do SDK 3.6.2 com o
// `_RegExp` trocado: o da VM chama o irregexp (natives `RegExp_*` e
// intrínsecos de registradores); este chama o motor do runtime
// (`DartForge_regexp_*`, `crates/runtime/src/regexp.rs`). `_RegExpMatch`,
// `RegExp.escape` e os iteradores de `allMatches` são os da VM.

// Copyright (c) 2012, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

part of "core_patch.dart";

@patch
class RegExp {
  @patch
  factory RegExp(String source,
      {bool multiLine = false,
      bool caseSensitive = true,
      bool unicode = false,
      bool dotAll = false}) {
    return new _RegExp(source,
        multiLine: multiLine,
        caseSensitive: caseSensitive,
        unicode: unicode,
        dotAll: dotAll);
  }

  /**
   * Finds the index of the first RegExp-significant char in [text].
   *
   * Starts looking from [start]. Returns `text.length` if no character
   * is found that has special meaning in RegExp syntax.
   */
  static int _findEscapeChar(String text, int start) {
    // Table where each character in the range U+0000 to U+007f is represented
    // by whether it needs to be escaped in a regexp.
    // The \x00 characters means escaped, and \x01 means non-escaped.
    const escapes =
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01"
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01"
        //                 $               (   )   *   +           .
        "\x01\x01\x01\x01\x00\x01\x01\x01\x00\x00\x00\x00\x01\x01\x00\x01"
        //                                                             ?
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x00"
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01"
        //                                             [   \   ]   ^
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x00\x00\x00\x00\x01"
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01"
        //                                             {   |   }
        "\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x01\x00\x00\x00\x01\x01";
    for (int i = start; i < text.length; i++) {
      int char = text.codeUnitAt(i);
      if (char <= 0x7f && escapes.codeUnitAt(char) == 0) return i;
    }
    return text.length;
  }

  @patch
  static String escape(String text) {
    int escapeCharIndex = _findEscapeChar(text, 0);
    // If the text contains no characters needing escape, return it directly.
    if (escapeCharIndex == text.length) return text;

    var buffer = new StringBuffer();
    int previousSliceEndIndex = 0;
    do {
      // Copy characters from previous escape to current escape into result.
      // This includes the previously escaped character.
      buffer.write(text.substring(previousSliceEndIndex, escapeCharIndex));
      // Prepare the current character to be escaped by prefixing it with a '\'.
      buffer.write(r"\");
      previousSliceEndIndex = escapeCharIndex;
      escapeCharIndex = _findEscapeChar(text, escapeCharIndex + 1);
    } while (escapeCharIndex < text.length);
    // Copy tail of string into result.
    buffer.write(text.substring(previousSliceEndIndex, escapeCharIndex));
    return buffer.toString();
  }

  int get _groupCount;
  Iterable<String> get _groupNames;
  int _groupNameIndex(String name);
}

class _RegExpMatch implements RegExpMatch {
  _RegExpMatch._(this._regexp, this.input, this._match);

  int get start => _start(0);
  int get end => _end(0);

  int _start(int groupIdx) {
    return _match[(groupIdx * _MATCH_PAIR)];
  }

  int _end(int groupIdx) {
    return _match[(groupIdx * _MATCH_PAIR) + 1];
  }

  String? group(int groupIdx) {
    if (groupIdx < 0 || groupIdx > _regexp._groupCount) {
      throw new RangeError.value(groupIdx);
    }
    int startIndex = _start(groupIdx);
    int endIndex = _end(groupIdx);
    if (startIndex == -1) {
      assert(endIndex == -1);
      return null;
    }
    return input._substringUnchecked(startIndex, endIndex);
  }

  String? operator [](int groupIdx) {
    return this.group(groupIdx);
  }

  List<String?> groups(List<int> groupsSpec) {
    var groupsList = new List<String?>.filled(groupsSpec.length, null);
    for (int i = 0; i < groupsSpec.length; i++) {
      groupsList[i] = group(groupsSpec[i]);
    }
    return groupsList;
  }

  int get groupCount => _regexp._groupCount;

  RegExp get pattern => _regexp;

  String? namedGroup(String name) {
    var idx = _regexp._groupNameIndex(name);
    if (idx < 0) {
      throw ArgumentError("Not a capture group name: ${name}");
    }
    return group(idx);
  }

  Iterable<String> get groupNames {
    return _regexp._groupNames;
  }

  final RegExp _regexp;
  final String input;
  final List<int> _match;
  static const int _MATCH_PAIR = 2;
}

/// O `RegExp` do backend nativo do DartForge. Mesma interface do `_RegExp`
/// da VM; o motor é o do runtime (`crates/runtime/src/regexp.rs`, semântica
/// do ECMAScript como o irregexp), com o padrão compilado uma vez num id e
/// as posições do último casamento lidas uma a uma.
@pragma("vm:entry-point")
class _RegExp implements RegExp {
  final String pattern;
  final bool isMultiLine;
  final bool isCaseSensitive;
  final bool isUnicode;
  final bool isDotAll;
  final int _id;
  final int _groupCount;

  factory _RegExp(String pattern,
      {bool multiLine = false,
      bool caseSensitive = true,
      bool unicode = false,
      bool dotAll = false}) {
    final id = _compilar(pattern, multiLine, caseSensitive, unicode, dotAll);
    if (id < 0) {
      throw FormatException("${_erro()} $pattern");
    }
    return _RegExp._(pattern, multiLine, caseSensitive, unicode, dotAll, id,
        _grupos(id));
  }

  _RegExp._(this.pattern, this.isMultiLine, this.isCaseSensitive,
      this.isUnicode, this.isDotAll, this._id, this._groupCount);

  @pragma("vm:external-name", "DartForge_regexp_compilar")
  external static int _compilar(String padrao, bool multiLinha, bool sensivel,
      bool unicode, bool pontoTudo);

  @pragma("vm:external-name", "DartForge_regexp_erro")
  external static String _erro();

  @pragma("vm:external-name", "DartForge_regexp_grupos")
  external static int _grupos(int id);

  @pragma("vm:external-name", "DartForge_regexp_n_nomes")
  external static int _nNomes(int id);

  @pragma("vm:external-name", "DartForge_regexp_nome")
  external static String _nome(int id, int i);

  @pragma("vm:external-name", "DartForge_regexp_indice_do_nome")
  external static int _indiceDoNome(int id, int i);

  @pragma("vm:external-name", "DartForge_regexp_executar")
  external static bool _executar(int id, String alvo, int inicio, bool pegajoso);

  @pragma("vm:external-name", "DartForge_regexp_captura")
  external static int _captura(int i);

  Iterable<String> get _groupNames sync* {
    final n = _nNomes(_id);
    for (var i = 0; i < n; i++) {
      yield _nome(_id, i);
    }
  }

  int _groupNameIndex(String name) {
    final n = _nNomes(_id);
    for (var i = 0; i < n; i++) {
      if (name == _nome(_id, i)) {
        return _indiceDoNome(_id, i);
      }
    }
    return -1;
  }

  List<int>? _casar(String str, int inicio, bool pegajoso) {
    if (!_executar(_id, str, inicio, pegajoso)) return null;
    return List<int>.generate(2 * (_groupCount + 1), _captura);
  }

  List<int>? _ExecuteMatch(String str, int start_index) =>
      _casar(str, start_index, false);

  List<int>? _ExecuteMatchSticky(String str, int start_index) =>
      _casar(str, start_index, true);

  RegExpMatch? firstMatch(String input) {
    final match = _ExecuteMatch(input, 0);
    if (match == null) {
      return null;
    }
    return new _RegExpMatch._(this, input, match);
  }

  Iterable<RegExpMatch> allMatches(String string, [int start = 0]) {
    if (0 > start || start > string.length) {
      throw new RangeError.range(start, 0, string.length);
    }
    return new _AllMatchesIterable(this, string, start);
  }

  RegExpMatch? matchAsPrefix(String string, [int start = 0]) {
    if (start < 0 || start > string.length) {
      throw new RangeError.range(start, 0, string.length);
    }
    final list = _ExecuteMatchSticky(string, start);
    if (list == null) return null;
    return new _RegExpMatch._(this, string, list);
  }

  bool hasMatch(String input) {
    return _ExecuteMatch(input, 0) != null;
  }

  String? stringMatch(String input) {
    List? match = _ExecuteMatch(input, 0);
    if (match == null) {
      return null;
    }
    return input._substringUnchecked(match[0], match[1]);
  }

  int get hashCode => pattern.hashCode;

  bool operator ==(Object other) {
    return other is _RegExp &&
        pattern == other.pattern &&
        isMultiLine == other.isMultiLine &&
        isCaseSensitive == other.isCaseSensitive &&
        isUnicode == other.isUnicode &&
        isDotAll == other.isDotAll;
  }

  String toString() => "RegExp: pattern=$pattern flags=${_flags()}";

  String _flags() {
    // A ordem do `RegExpFlags::ToCString` da VM.
    var s = "";
    if (!isCaseSensitive) s += "i";
    if (isMultiLine) s += "m";
    if (isUnicode) s += "u";
    if (isDotAll) s += "s";
    return s;
  }
}

class _AllMatchesIterable extends Iterable<RegExpMatch> {
  final _RegExp _re;
  final String _str;
  final int _start;

  _AllMatchesIterable(this._re, this._str, this._start);

  Iterator<RegExpMatch> get iterator =>
      new _AllMatchesIterator(_re, _str, _start);
}

class _AllMatchesIterator implements Iterator<RegExpMatch> {
  final String _str;
  int _nextIndex;
  _RegExp? _re;
  RegExpMatch? _current;

  _AllMatchesIterator(this._re, this._str, this._nextIndex);

  RegExpMatch get current => _current as RegExpMatch;

  static bool _isLeadSurrogate(int c) {
    return c >= 0xd800 && c <= 0xdbff;
  }

  static bool _isTrailSurrogate(int c) {
    return c >= 0xdc00 && c <= 0xdfff;
  }

  bool moveNext() {
    final re = _re;
    if (re == null) return false; // Cleared after a failed match.
    if (_nextIndex <= _str.length) {
      final match = re._ExecuteMatch(_str, _nextIndex);
      if (match != null) {
        var current = new _RegExpMatch._(re, _str, match);
        _current = current;
        _nextIndex = current.end;
        if (_nextIndex == current.start) {
          // Zero-width match. Advance by one more, unless the regexp
          // is in unicode mode and it would put us within a surrogate
          // pair. In that case, advance past the code point as a whole.
          if (re.isUnicode &&
              _nextIndex + 1 < _str.length &&
              _isLeadSurrogate(_str.codeUnitAt(_nextIndex)) &&
              _isTrailSurrogate(_str.codeUnitAt(_nextIndex + 1))) {
            _nextIndex++;
          }
          _nextIndex++;
        }
        return true;
      }
    }
    _current = null;
    _re = null;
    return false;
  }
}
