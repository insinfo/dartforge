/// O canal do executor de builders: quadros `u32` big-endian + JSON em UTF-8
/// por stdin/stdout (docs/BUILD-PROTOCOLO.md §1). stderr fica livre para log
/// humano; nada além dos quadros pode ir ao stdout.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

/// Mensagens JSON já enquadradas. Os testes usam um canal em memória.
abstract interface class Canal {
  Stream<Map<String, Object?>> get entrada;
  void enviar(Map<String, Object?> mensagem);
  Future<void> fechar();
}

/// O canal de stdio do processo executor.
final class CanalStdio implements Canal {
  final _saida = StreamController<Map<String, Object?>>();

  CanalStdio() {
    var pendente = Uint8List(0);
    stdin.listen((bytes) {
      final dados = Uint8List(pendente.length + bytes.length)
        ..setAll(0, pendente)
        ..setAll(pendente.length, bytes);
      var i = 0;
      while (dados.length - i >= 4) {
        final n = ByteData.sublistView(dados, i, i + 4).getUint32(0, Endian.big);
        if (dados.length - i - 4 < n) break;
        final texto = utf8.decode(Uint8List.sublistView(dados, i + 4, i + 4 + n));
        _saida.add(jsonDecode(texto) as Map<String, Object?>);
        i += 4 + n;
      }
      pendente = Uint8List.fromList(Uint8List.sublistView(dados, i));
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
