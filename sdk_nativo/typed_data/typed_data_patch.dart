// Substitui `_internal/vm/lib/typed_data_patch.dart` (sobreposição
// `sdk_nativo/`, P5c/P5d, docs/NATIVO-PLANO.md §7).
//
// As listas tipadas da VM guardam os bytes num objeto interno e os leem por
// intrínsecos por tipo de elemento (`_getUint32`…). Aqui cada lista tipada
// guarda os elementos numa lista de tamanho fixo e ajusta o valor gravado ao
// tipo do elemento (corte de bits, sinal, saturação do `Clamped`), que é o
// que o programa observa: `[]`, `[]=`, `length`, `sublist`, `is Uint8List`.
// Ainda não há `ByteBuffer`/`ByteData`, visões, `Float32List` nem SIMD: esses
// membros continuam `external` sem implementação, e o membro que os usa é
// recusado com o motivo (nunca uma resposta errada).

import "dart:_internal" show patch, FixedLengthListMixin;
import "dart:collection" show ListBase;

/// A base das listas de inteiros: os elementos e o ajuste ao tipo.
abstract base class _ListaDeInteiros extends ListBase<int>
    with FixedLengthListMixin<int> {
  final List<int> _elementos;

  _ListaDeInteiros(int length) : _elementos = List<int>.filled(length, 0);

  int get length => _elementos.length;

  int operator [](int index) => _elementos[index];

  void operator []=(int index, int value) {
    _elementos[index] = _ajustar(value);
  }

  /// O valor como o tipo do elemento o guarda.
  int _ajustar(int v);

  int get elementSizeInBytes;

  int get offsetInBytes => 0;

  int get lengthInBytes => length * elementSizeInBytes;

  ByteBuffer get buffer =>
      throw UnsupportedError("ByteBuffer no backend nativo do DartForge");

  /// Os elementos de `start` a `end` em `nova`, do mesmo tipo.
  T _fatia<T extends _ListaDeInteiros>(int start, int? end, T nova(int n)) {
    final fim = RangeError.checkValidRange(start, end, length);
    final r = nova(fim - start);
    for (int i = start; i < fim; i++) {
      r._elementos[i - start] = _elementos[i];
    }
    return r;
  }
}
@patch
class Int8List {
  @patch
  factory Int8List(int length) => _Int8ListaDF(length);

  @patch
  factory Int8List.fromList(List<int> elements) {
    final r = _Int8ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Int8ListaDF extends _ListaDeInteiros implements Int8List {
  _Int8ListaDF(int length) : super(length);

  int _ajustar(int v) => (v & 0xFF) >= 0x80 ? (v & 0xFF) - 0x100 : (v & 0xFF);

  int get elementSizeInBytes => 1;

  Int8List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Int8ListaDF(n));

  Int8List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Uint8List {
  @patch
  factory Uint8List(int length) => _Uint8ListaDF(length);

  @patch
  factory Uint8List.fromList(List<int> elements) {
    final r = _Uint8ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Uint8ListaDF extends _ListaDeInteiros implements Uint8List {
  _Uint8ListaDF(int length) : super(length);

  int _ajustar(int v) => v & 0xFF;

  int get elementSizeInBytes => 1;

  Uint8List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Uint8ListaDF(n));

  Uint8List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Uint8ClampedList {
  @patch
  factory Uint8ClampedList(int length) => _Uint8ClampedListaDF(length);

  @patch
  factory Uint8ClampedList.fromList(List<int> elements) {
    final r = _Uint8ClampedListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Uint8ClampedListaDF extends _ListaDeInteiros implements Uint8ClampedList {
  _Uint8ClampedListaDF(int length) : super(length);

  int _ajustar(int v) => v < 0 ? 0 : (v > 255 ? 255 : v);

  int get elementSizeInBytes => 1;

  Uint8ClampedList sublist(int start, [int? end]) => _fatia(start, end, (n) => _Uint8ClampedListaDF(n));

  Uint8ClampedList asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Int16List {
  @patch
  factory Int16List(int length) => _Int16ListaDF(length);

  @patch
  factory Int16List.fromList(List<int> elements) {
    final r = _Int16ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Int16ListaDF extends _ListaDeInteiros implements Int16List {
  _Int16ListaDF(int length) : super(length);

  int _ajustar(int v) => (v & 0xFFFF) >= 0x8000 ? (v & 0xFFFF) - 0x10000 : (v & 0xFFFF);

  int get elementSizeInBytes => 2;

  Int16List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Int16ListaDF(n));

  Int16List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Uint16List {
  @patch
  factory Uint16List(int length) => _Uint16ListaDF(length);

  @patch
  factory Uint16List.fromList(List<int> elements) {
    final r = _Uint16ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Uint16ListaDF extends _ListaDeInteiros implements Uint16List {
  _Uint16ListaDF(int length) : super(length);

  int _ajustar(int v) => v & 0xFFFF;

  int get elementSizeInBytes => 2;

  Uint16List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Uint16ListaDF(n));

  Uint16List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Int32List {
  @patch
  factory Int32List(int length) => _Int32ListaDF(length);

  @patch
  factory Int32List.fromList(List<int> elements) {
    final r = _Int32ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Int32ListaDF extends _ListaDeInteiros implements Int32List {
  _Int32ListaDF(int length) : super(length);

  int _ajustar(int v) => (v & 0xFFFFFFFF) >= 0x80000000 ? (v & 0xFFFFFFFF) - 0x100000000 : (v & 0xFFFFFFFF);

  int get elementSizeInBytes => 4;

  Int32List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Int32ListaDF(n));

  Int32List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Uint32List {
  @patch
  factory Uint32List(int length) => _Uint32ListaDF(length);

  @patch
  factory Uint32List.fromList(List<int> elements) {
    final r = _Uint32ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Uint32ListaDF extends _ListaDeInteiros implements Uint32List {
  _Uint32ListaDF(int length) : super(length);

  int _ajustar(int v) => v & 0xFFFFFFFF;

  int get elementSizeInBytes => 4;

  Uint32List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Uint32ListaDF(n));

  Uint32List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Int64List {
  @patch
  factory Int64List(int length) => _Int64ListaDF(length);

  @patch
  factory Int64List.fromList(List<int> elements) {
    final r = _Int64ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Int64ListaDF extends _ListaDeInteiros implements Int64List {
  _Int64ListaDF(int length) : super(length);

  int _ajustar(int v) => v;

  int get elementSizeInBytes => 8;

  Int64List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Int64ListaDF(n));

  Int64List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Uint64List {
  @patch
  factory Uint64List(int length) => _Uint64ListaDF(length);

  @patch
  factory Uint64List.fromList(List<int> elements) {
    final r = _Uint64ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Uint64ListaDF extends _ListaDeInteiros implements Uint64List {
  _Uint64ListaDF(int length) : super(length);

  int _ajustar(int v) => v;

  int get elementSizeInBytes => 8;

  Uint64List sublist(int start, [int? end]) => _fatia(start, end, (n) => _Uint64ListaDF(n));

  Uint64List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}

@patch
class Float64List {
  @patch
  factory Float64List(int length) => _Float64ListaDF(length);

  @patch
  factory Float64List.fromList(List<double> elements) {
    final r = _Float64ListaDF(elements.length);
    for (int i = 0; i < elements.length; i++) {
      r[i] = elements[i];
    }
    return r;
  }
}

final class _Float64ListaDF extends ListBase<double>
    with FixedLengthListMixin<double>
    implements Float64List {
  final List<double> _elementos;

  _Float64ListaDF(int length) : _elementos = List<double>.filled(length, 0.0);

  int get length => _elementos.length;

  double operator [](int index) => _elementos[index];

  void operator []=(int index, double value) {
    _elementos[index] = value;
  }

  int get elementSizeInBytes => 8;

  int get offsetInBytes => 0;

  int get lengthInBytes => length * 8;

  ByteBuffer get buffer =>
      throw UnsupportedError("ByteBuffer no backend nativo do DartForge");

  Float64List sublist(int start, [int? end]) {
    final fim = RangeError.checkValidRange(start, end, length);
    final r = _Float64ListaDF(fim - start);
    for (int i = start; i < fim; i++) {
      r._elementos[i - start] = _elementos[i];
    }
    return r;
  }

  Float64List asUnmodifiableView() =>
      throw UnsupportedError("visão não modificável no backend nativo do DartForge");
}
