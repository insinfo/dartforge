// Substitui `_internal/vm_shared/lib/string_buffer_patch.dart` (sobreposição
// `sdk_nativo/`, P5c). O da VM junta as partes numa lista e as unidades
// soltas num `Uint16List`, compacta a lista de tempos em tempos e cria a
// string por um native. Aqui as unidades escritas vão direto para um
// acumulador do runtime (`_AcumuladorDeTexto`, um objeto com o vetor de
// unidades anexado, solto pelo coletor com ele; `nativos_strings.rs`): uma
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
    // A string vai ao acumulador numa chamada só (o teste `is String`, o
    // `isEmpty` e o `length` ficam no runtime); o resto passa pelo
    // `toString`, como na VM.
    final int n = _escreverSeTexto(_acumulador ??= _novo(), obj);
    if (n >= 0) {
      _unidades += n;
      return;
    }
    String str = "$obj";
    if (str.isEmpty) return;
    _escrever(_acumulador!, str);
    _unidades += str.length;
  }

  // As unidades vão direto ao acumulador, sem a string de um caractere que
  // o `String.fromCharCode` alocava (o `writeCharCode` do
  // `_JsonStringifier` e do `_JsonStringParser` escreve por aqui); as
  // conferências e o `RangeError` são os da VM.
  @patch
  void writeCharCode(int charCode) {
    if (charCode <= 0xFFFF) {
      if (charCode < 0) {
        throw new RangeError.range(charCode, 0, 0x10FFFF);
      }
      _escreverCodigo(_acumulador ??= _novo(), charCode);
      _unidades += 1;
    } else {
      if (charCode > 0x10FFFF) {
        throw new RangeError.range(charCode, 0, 0x10FFFF);
      }
      _escreverCodigo(_acumulador ??= _novo(), charCode);
      _unidades += 2;
    }
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

  /// Se `obj` é uma `String`, acrescenta as unidades dela e devolve quantas;
  /// senão -1 (sem mudar nada).
  @pragma("vm:external-name", "DartForge_sb_escrever_se_texto")
  external static int _escreverSeTexto(Object acumulador, Object? obj);

  /// Acrescenta o ponto de código `codigo` (0..0x10FFFF, conferido pelo
  /// Dart): uma unidade, ou o par de surrogates acima de 0xFFFF.
  @pragma("vm:external-name", "DartForge_sb_escrever_codigo")
  external static void _escreverCodigo(Object acumulador, int codigo);

  @pragma("vm:external-name", "DartForge_sb_texto")
  external static String _texto(Object acumulador);
}
