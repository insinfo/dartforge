// Substitui `_internal/vm_shared/lib/string_buffer_patch.dart` (sobreposição
// `sdk_nativo/`, P5c). O da VM junta as partes numa lista e as unidades
// soltas num `Uint16List`, compacta a lista de tempos em tempos e cria a
// string por um native. Aqui as unidades escritas vão direto para um
// acumulador do runtime (`Value::StringBuffer`, `nativos_strings.rs`): uma
// chamada por `write`, sem um objeto por parte e sem a conferência de
// covariância do `add` da lista; o `toString` copia o acumulado para uma
// string nova (o `StringBuffer` continua podendo crescer). O que o programa
// observa — o texto, `length`, `isEmpty` — é o mesmo.

import "dart:_internal" show patch;

@patch
class StringBuffer {
  /// O acumulador do runtime; `null` enquanto vazio.
  Object? _acumulador;

  /// As unidades de código escritas.
  int _unidades = 0;

  @patch
  StringBuffer([Object content = ""]) {
    write(content);
  }

  @patch
  int get length => _unidades;

  @patch
  void write(Object? obj) {
    String str = obj is String ? obj : "$obj";
    if (str.isEmpty) return;
    _escrever(_acumulador ??= _novo(), str);
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
    _acumulador = null;
    _unidades = 0;
  }

  @patch
  String toString() {
    final acumulador = _acumulador;
    if (acumulador == null) return "";
    return _texto(acumulador);
  }

  @pragma("vm:external-name", "DartForge_sb_novo")
  external static Object _novo();

  @pragma("vm:external-name", "DartForge_sb_escrever")
  external static void _escrever(Object acumulador, String str);

  @pragma("vm:external-name", "DartForge_sb_texto")
  external static String _texto(Object acumulador);
}
