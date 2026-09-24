// Oráculo das macros (docs/MACROS-PROTOCOLO.md §7): extrai de um `.dill`
// compilado pelo CFE 3.6.2 com `--enable-experiment=macros` o texto da
// biblioteca de augmentation **fundida** que o CFE gerou
// (`buildMergedAugmentationLibraries`, que grava o texto na tabela de fontes
// do componente sob a URI `dart-macro+<uri da biblioteca>`).
//
// Uso (com o `dart` do SDK 3.6.2, o oráculo):
//   dart compile kernel --enable-experiment=macros bin/main.dart -o main.dill
//   dart scripts/oraculo_augmentation.dart main.dill [saida_dir]
//
// Para cada biblioteca aumentada imprime (ou grava em `saida_dir/<arquivo>.
// augmentation.dart`) o texto byte a byte. A leitura é da tabela de fontes do
// formato binário do kernel (`pkg/kernel/binary.md`: `Source { List<Byte>
// uriUtf8Bytes; List<Byte> sourceUtf8Bytes; … }`, `List<Byte>` = `UInt`
// + bytes, `UInt` de 1, 2 ou 4 bytes pelos dois bits altos): não depende de
// `package:kernel`, que não vem na distribuição do SDK.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

void main(List<String> args) {
  if (args.isEmpty) {
    stderr.writeln('uso: dart oraculo_augmentation.dart <arquivo.dill> [saida_dir]');
    exit(2);
  }
  final bytes = File(args[0]).readAsBytesSync();
  final saida = args.length > 1 ? Directory(args[1]) : null;
  final prefixo = utf8.encode('dart-macro+');
  var achados = 0;
  for (var i = 0; i + prefixo.length < bytes.length; i++) {
    if (!_casa(bytes, i, prefixo)) continue;
    // `i` é o começo dos bytes da URI; o comprimento vem logo antes.
    final uri = _lerLista(bytes, i);
    if (uri == null) continue;
    final (uriTexto, fimUri) = uri;
    // A fonte vem logo depois da URI, com o seu próprio comprimento.
    final fonte = _lerListaEm(bytes, fimUri);
    if (fonte == null) continue;
    final texto = fonte.$1;
    if (texto.isEmpty) continue;
    achados++;
    if (saida == null) {
      stdout.writeln('// ${uriTexto}');
      stdout.write(texto);
      stdout.writeln();
    } else {
      final nome = Uri.parse(uriTexto.substring('dart-macro+'.length))
          .pathSegments
          .last
          .replaceAll('.dart', '.augmentation.dart');
      saida.createSync(recursive: true);
      File('${saida.path}/$nome').writeAsStringSync(texto);
      stdout.writeln('${saida.path}/$nome');
    }
    i = fimUri;
  }
  if (achados == 0) {
    stderr.writeln('nenhuma biblioteca de augmentation de macro no componente');
    exit(1);
  }
}

bool _casa(Uint8List b, int i, List<int> p) {
  for (var k = 0; k < p.length; k++) {
    if (b[i + k] != p[k]) return false;
  }
  return true;
}

/// Lê a `List<Byte>` cujos bytes começam em [inicio], achando o `UInt` do
/// comprimento imediatamente antes (1, 2 ou 4 bytes).
(String, int)? _lerLista(Uint8List b, int inicio) {
  for (final largura in const [1, 2, 4]) {
    final p = inicio - largura;
    if (p < 0) continue;
    final lido = _uint(b, p);
    if (lido == null || lido.$2 != inicio) continue;
    final fim = inicio + lido.$1;
    if (fim > b.length) continue;
    try {
      return (utf8.decode(b.sublist(inicio, fim)), fim);
    } on FormatException {
      continue;
    }
  }
  return null;
}

(String, int)? _lerListaEm(Uint8List b, int p) {
  final lido = _uint(b, p);
  if (lido == null) return null;
  final fim = lido.$2 + lido.$1;
  if (fim > b.length) return null;
  try {
    return (utf8.decode(b.sublist(lido.$2, fim)), fim);
  } on FormatException {
    return null;
  }
}

/// `UInt` do kernel: `0xxxxxxx`, `10xxxxxx xxxxxxxx` ou `11xxxxxx` + 3 bytes.
/// Devolve (valor, posição seguinte).
(int, int)? _uint(Uint8List b, int p) {
  final x = b[p];
  if (x & 0x80 == 0) return (x, p + 1);
  if (x & 0xc0 == 0x80) {
    if (p + 2 > b.length) return null;
    return (((x & 0x3f) << 8) | b[p + 1], p + 2);
  }
  if (p + 4 > b.length) return null;
  return (((x & 0x3f) << 24) | (b[p + 1] << 16) | (b[p + 2] << 8) | b[p + 3], p + 4);
}
