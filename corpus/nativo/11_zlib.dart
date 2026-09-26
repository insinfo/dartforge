// Os filtros zlib/gzip do dart:io: a mesma saída comprimida da VM.
import 'dart:convert';
import 'dart:io';

Future<void> main() async {
  final dados = utf8.encode('olá dartforge ' * 100);
  final z = zlib.encode(dados);
  print('zlib ${z.length} ${utf8.decode(zlib.decode(z)) == 'olá dartforge ' * 100}');
  final g = gzip.encode(dados);
  print('gzip ${g[0]} ${g[1]} ${gzip.decode(g).length}');
  final raw = ZLibCodec(raw: true, level: 9);
  print('raw ${raw.encode(dados).length} ${raw.decode(raw.encode(dados)).length}');
  final partes = await Stream<List<int>>.fromIterable([dados.sublist(0, 700), dados.sublist(700)])
      .transform(gzip.encoder)
      .transform(gzip.decoder)
      .toList();
  print('stream ${partes.expand((p) => p).length}');
  print(zlib.decode(zlib.encode(<int>[1, 2, 3, 300])));
  try {
    zlib.decode([1, 2, 3, 4, 5]);
  } on FormatException catch (e) {
    print('erro: ${e.message}');
  }
}
