/// O canal do processo executor: quadros `u32` big-endian + JSON em UTF-8
/// por stdin/stdout (docs/BUILD-PROTOCOLO.md §1, o enquadramento do
/// `message_grouper.dart` do protótipo oficial). stderr fica livre para log.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'servico.dart';

final class CanalStdio implements Canal {
  final _saida = StreamController<Map<String, Object?>>();

  CanalStdio() {
    final buffer = BytesBuilder(copy: false);
    stdin.listen((bytes) {
      buffer.add(bytes);
      var dados = buffer.takeBytes();
      var i = 0;
      while (dados.length - i >= 4) {
        final n = ByteData.sublistView(dados, i, i + 4).getUint32(0, Endian.big);
        if (dados.length - i - 4 < n) break;
        final json = utf8.decode(Uint8List.sublistView(dados, i + 4, i + 4 + n));
        _saida.add(jsonDecode(json) as Map<String, Object?>);
        i += 4 + n;
      }
      if (i < dados.length) buffer.add(Uint8List.sublistView(dados, i));
    }, onDone: _saida.close);
  }

  @override
  Stream<Map<String, Object?>> get entrada => _saida.stream;

  @override
  void enviar(Map<String, Object?> mensagem) {
    final corpo = utf8.encode(jsonEncode(mensagem));
    final cabecalho = ByteData(4)..setUint32(0, corpo.length, Endian.big);
    stdout.add(cabecalho.buffer.asUint8List());
    stdout.add(corpo);
  }

  @override
  Future<void> fechar() => stdout.flush();
}
