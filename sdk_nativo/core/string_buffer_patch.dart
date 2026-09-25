// Substitui `_internal/vm_shared/lib/string_buffer_patch.dart` (sobreposição
// `sdk_nativo/`, P5c). O da VM acumula as unidades num `Uint16List` e cria a
// string por um native (`StringBuffer_createStringFromUint16Array`); aqui as
// partes escritas ficam numa lista e o `toString` as concatena de uma vez
// (`_StringBase._concatRange`, o native `String_concatRange`). O que o
// programa observa — o texto, `length`, `isEmpty` — é o mesmo.

import "dart:_internal" show patch;

@patch
class StringBuffer {
  /// As partes escritas, na ordem; `null` enquanto vazio.
  List<String>? _partes;

  /// A soma das unidades de código das partes.
  int _unidades = 0;

  @patch
  StringBuffer([Object content = ""]) {
    write(content);
  }

  @patch
  int get length => _unidades;

  @patch
  void write(Object? obj) {
    String str = "$obj";
    if (str.isEmpty) return;
    (_partes ??= <String>[]).add(str);
    _unidades += str.length;
  }

  @patch
  void writeCharCode(int charCode) {
    write(String.fromCharCode(charCode));
  }

  @patch
  void writeAll(Iterable objects, [String separator = ""]) {
    Iterator iterator = objects.iterator;
    if (!iterator.moveNext()) return;
    if (separator.isEmpty) {
      do {
        write(iterator.current);
      } while (iterator.moveNext());
    } else {
      write(iterator.current);
      while (iterator.moveNext()) {
        write(separator);
        write(iterator.current);
      }
    }
  }

  @patch
  void writeln([Object? obj = ""]) {
    write(obj);
    write("\n");
  }

  @patch
  void clear() {
    _partes = null;
    _unidades = 0;
  }

  @patch
  String toString() {
    final partes = _partes;
    if (partes == null) return "";
    return _StringBase._concatRange(partes, 0, partes.length);
  }
}
