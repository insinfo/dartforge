// UTF-8, latin-1 e bytes no espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md
// §5.3): `utf8.encode`/`decode` (sequências inválidas com e sem `allowMalformed`,
// BOM, substitutos soltos e pares), `latin1` (e o erro acima de 0xFF),
// `String.fromCharCodes` de `Uint8List`, de uma visão e de um intervalo, e
// `json.fuse(utf8)` nos dois sentidos.
import 'dart:convert';
import 'dart:typed_data';

String hex(List<int> b) => b.map((x) => x.toRadixString(16).padLeft(2, '0')).join(' ');

String unidades(String s) => s.codeUnits.map((c) => c.toRadixString(16)).join(' ');

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } on FormatException catch (e) {
    print('$nome: FormatException ${e.message} offset=${e.offset}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

void main() {
  // encode de todas as larguras.
  for (final s in ['', 'abc', 'ÿ', 'ação', 'Ā', '€', '日本', '😀', 'a😀b\u0000z']) {
    final b = utf8.encode(s);
    print('encode "${s.replaceAll('\u0000', r'\0')}": ${b.runtimeType} ${b.length} [${hex(b)}]');
    print('  volta: ${utf8.decode(b) == s}');
  }

  // Substitutos soltos no encode viram EF BF BD.
  final solto = 'x${String.fromCharCode(0xD800)}y${String.fromCharCode(0xDC00)}';
  print('encode soltos: [${hex(utf8.encode(solto))}]');
  print('decode dos soltos: [${unidades(utf8.decode(utf8.encode(solto)))}]');
  final invertido = String.fromCharCodes([0xDE00, 0xD83D]);
  print('encode invertido: [${hex(utf8.encode(invertido))}]');

  // Decodificação de sequências inválidas.
  final invalidas = <String, List<int>>{
    'continuação solta': [0x61, 0x80, 0x62],
    'truncada': [0xE6, 0x97],
    'overlong': [0xC0, 0xAF],
    'surrogate codificado': [0xED, 0xA0, 0x80],
    'acima de 10FFFF': [0xF4, 0x90, 0x80, 0x80],
    'byte FF': [0xFF, 0x41],
    'par de 4 bytes truncado no fim': [0x41, 0xF0, 0x9F, 0x98],
  };
  invalidas.forEach((nome, bytes) {
    tentar('decode $nome', () => utf8.decode(bytes));
    tentar('  allowMalformed', () => unidades(utf8.decode(bytes, allowMalformed: true)));
    tentar('  Utf8Decoder', () => unidades(const Utf8Decoder(allowMalformed: true).convert(bytes)));
  });

  // BOM: o utf8.decode da VM descarta o U+FEFF inicial; o encode o preserva.
  final comBom = [0xEF, 0xBB, 0xBF, 0x6F, 0x69];
  final dec = utf8.decode(comBom);
  final bom = String.fromCharCode(0xFEFF);
  print('BOM: len=${dec.length} [${unidades(dec)}] começa com FEFF: ${dec.startsWith(bom)}');
  print('BOM encode: [${hex(utf8.encode('${bom}oi'))}]');
  final bomNoMeio = utf8.decode([0x6F, 0xEF, 0xBB, 0xBF, 0x69]);
  print('BOM no meio: [${unidades(bomNoMeio)}]');
  print('BOM duplo: [${unidades(utf8.decode([...comBom.take(3), ...comBom]))}]');

  // Decodificação de intervalos e de visões.
  final bytes = Uint8List.fromList(utf8.encode('prefixo|olá 😀 mundo|sufixo'));
  final inicio = bytes.indexOf(0x7C) + 1;
  final fim = bytes.lastIndexOf(0x7C);
  print('convert intervalo: ${utf8.decoder.convert(bytes, inicio, fim)}');
  final visao = Uint8List.sublistView(bytes, inicio, fim);
  print('decode sublistView: ${utf8.decode(visao)} (${visao.length} bytes, offset ${visao.offsetInBytes})');
  final visao2 = bytes.buffer.asUint8List(bytes.offsetInBytes + inicio, 4);
  print('decode asUint8List: ${utf8.decode(visao2)}');
  tentar('decode no meio do emoji', () => utf8.decode(Uint8List.sublistView(bytes, fim - 8, fim - 6)));

  // latin1.
  print('latin1.encode: [${hex(latin1.encode('ação ÿ'))}]');
  print('latin1.decode: ${latin1.decode([0x63, 0xE9, 0x75, 0xFF])}');
  tentar('latin1.encode Ā', () => latin1.encode('aĀ'));
  tentar('latin1.decode 300', () => latin1.decode([0x61, 300]));
  print('latin1 allowInvalid: ${unidades(const Latin1Decoder(allowInvalid: true).convert([0x61, 300]))}');
  tentar('ascii.encode ç', () => ascii.encode('ç'));
  print('ascii allowInvalid: ${unidades(const AsciiDecoder(allowInvalid: true).convert([0x41, 0xC3]))}');

  // String.fromCharCodes de listas tipadas.
  final todos = Uint8List(256);
  for (var i = 0; i < 256; i++) {
    todos[i] = i;
  }
  final s256 = String.fromCharCodes(todos);
  print('fromCharCodes 0..255: len=${s256.length} ${s256.codeUnitAt(0)} ${s256.codeUnitAt(255)} '
      'soma=${s256.codeUnits.fold<int>(0, (a, b) => a + b)}');
  final vista = Uint8List.view(todos.buffer, 0x41, 26);
  print('fromCharCodes view: ${String.fromCharCodes(vista)}');
  print('fromCharCodes sublistView: ${String.fromCharCodes(Uint8List.sublistView(todos, 0x61, 0x61 + 26))}');
  print('fromCharCodes intervalo: ${String.fromCharCodes(todos, 0x30, 0x3A)}');
  final u16 = Uint16List.fromList([0x48, 0x100, 0xD83D, 0xDE00, 0x21]);
  print('fromCharCodes Uint16List: ${String.fromCharCodes(u16)} len=${String.fromCharCodes(u16).length}');
  final u32 = Uint32List.fromList([0x1F389, 0x41, 0xFF]);
  print('fromCharCodes Uint32List: [${unidades(String.fromCharCodes(u32))}]');
  final grande = Uint8List(100000);
  for (var i = 0; i < grande.length; i++) {
    grande[i] = 0x61 + i % 26;
  }
  final sg = String.fromCharCodes(grande);
  print('fromCharCodes grande: ${sg.length} ${sg.substring(99990)}');
  final utfGrande = utf8.encode('ç' * 50000);
  print('utf8 grande: ${utfGrande.length} ${utf8.decode(utfGrande).length}');

  // json.fuse(utf8).
  final codec = json.fuse(utf8);
  final obj = {
    'nome': 'São Paulo 😀',
    'n': [1, 2.5, -3, null, true],
    'aninhado': {'ÿ': 'Ā', 'vazio': ''},
  };
  final jb = codec.encode(obj);
  print('json.fuse encode: ${jb.length} bytes, começa [${hex(jb.take(12).toList())}]');
  final volta = codec.decode(jb) as Map;
  print('json.fuse decode: $volta');
  print('json.fuse igual: ${json.encode(volta) == json.encode(obj)}');
  final escapaveis = String.fromCharCodes([0x01, 0x2028, 0x22, 0x5C, 0xD800]);
  print('json com escape: [${hex(codec.encode(escapaveis))}]');
  tentar('json.fuse inválido', () => codec.decode([0x7B, 0x22, 0xFF, 0x22]));
  tentar('json.fuse truncado', () => codec.decode(utf8.encode('{"a": [1, 2')));
}
