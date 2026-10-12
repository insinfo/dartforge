// O cache do cabeçalho de lista tipada lida de campo (`lower/tipados.rs`,
// `cabecalho_tipado`): o campo trocado no meio do laço (crescimento, como
// o `_CopyingBytesBuilder`), listas descartadas com a coleta no meio (o
// handle de uma lista morta pode ser reusado), visão no campo, campo nulo,
// e o molde do `_HttpParser` (`_buffer![_index++]`). A saída tem de ser a
// da VM.
import 'dart:typed_data';

class Construtor {
  Uint8List _buffer = Uint8List(4);
  int _length = 0;

  void addByte(int b) {
    if (_buffer.length == _length) _grow(_length);
    _buffer[_length] = b;
    _length++;
  }

  void add(List<int> bytes) {
    final required = _length + bytes.length;
    if (_buffer.length < required) _grow(required);
    for (var i = 0; i < bytes.length; i++) {
      _buffer[_length + i] = bytes[i];
    }
    _length = required;
  }

  void _grow(int required) {
    var n = required * 2;
    if (n < 4) n = 4;
    final novo = Uint8List(n);
    novo.setRange(0, _buffer.length, _buffer);
    _buffer = novo;
    // Lixo para a coleta reusar handles.
    for (var k = 0; k < 50; k++) {
      Uint8List(64)[0] = k;
    }
  }

  List<int> takeBytes() => Uint8List.sublistView(_buffer, 0, _length);
}

class Leitor {
  Uint8List? _buffer;
  int _index = 0;
  int soma = 0;
  int linhas = 0;

  void alimentar(Uint8List dados) {
    _buffer = dados;
    _index = 0;
    while (_index < _buffer!.length) {
      final byte = _buffer![_index++];
      if (byte == 10) {
        linhas++;
        _trocarSeGrande();
      }
      soma = (soma * 31 + byte) & 0xFFFFFF;
    }
    _buffer = null;
  }

  void _trocarSeGrande() {
    // Troca o buffer no meio do laço por uma cópia (outro handle).
    if (linhas % 3 == 0 && _buffer != null) {
      _buffer = Uint8List.fromList(_buffer!);
    }
  }
}

class ComVisao {
  Uint16List dados = Uint16List.view(Uint8List(16).buffer, 4, 4);
  int ler(int i) => dados[i];
  void gravar(int i, int v) {
    dados[i] = v;
  }
}

void main() {
  final c = Construtor();
  for (var i = 0; i < 1000; i++) {
    c.addByte(i & 0xFF);
    if (i % 97 == 0) c.add('cabecalho: $i\r\n'.codeUnits);
  }
  final bytes = c.takeBytes();
  var h = 0;
  for (final b in bytes) {
    h = (h * 33 + b) & 0x3FFFFFFF;
  }
  print('${bytes.length} $h');

  final l = Leitor();
  for (var r = 0; r < 20; r++) {
    l.alimentar(Uint8List.fromList('GET /$r HTTP/1.1\nHost: x\nA: b\n\n'.codeUnits));
  }
  print('${l.linhas} ${l.soma}');
  print(() {
    try {
      l.alimentar(Uint8List(0));
      return l._buffer;
    } catch (e) {
      return e.runtimeType;
    }
  }());

  final v = ComVisao();
  for (var i = 0; i < 4; i++) {
    v.gravar(i, 1000 * i + 7);
  }
  print([for (var i = 0; i < 4; i++) v.ler(i)]);
  v.dados = Uint16List(4)..[2] = 42;
  print([for (var i = 0; i < 4; i++) v.ler(i)]);
  try {
    v.ler(9);
  } catch (e) {
    print(e.runtimeType);
  }
}
