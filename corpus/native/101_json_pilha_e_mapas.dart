// `jsonDecode` pela pilha única do listener e o mapa de cada objeto montado em
// lote (docs/NATIVO-PLANO.md §13): números extremos, strings com escapes,
// Unicode e pares substitutos (soltos também), aninhamento profundo, chaves
// repetidas, reviver, `json.fuse(utf8)` em bytes e em pedaços, e os erros de
// formato, com as posições.
import 'dart:convert';

String tipo(Object? v) {
  if (v is Map) return 'Map<${v is Map<String, dynamic>}>';
  if (v is List) return 'List<${v is List<dynamic>}>';
  return v.runtimeType.toString();
}

void numeros() {
  const texto = '[0, -0, 1, -1, 9007199254740993, 9223372036854775807, -9223372036854775808,'
      ' 9223372036854775808, -9223372036854775809, 1e308, 1e309, -1e309, 5e-324, 2e-324,'
      ' 0.1, 1.7976931348623157e308, 123456789012345678901234567890, 1E2, 1e-7, -0.0,'
      ' 0.30000000000000004, 2.2250738585072014e-308, 4.9e-324, 1e22, 1e23, 123.456e-2]';
  final l = jsonDecode(texto) as List;
  for (final v in l) {
    print('  ${v.runtimeType} $v ${v is double && v.isNegative}');
  }
  print('reencode: ${jsonEncode(l.where((v) => v is! double || v.isFinite).toList())}');
}

void textos() {
  final fontes = [
    r'"simples"',
    r'""',
    r'"aspas \" barra \\ barra / \/ controle \b\f\n\r\t"',
    r'"unicode éç 中文 😀 solto \ud800 fim \udc00"',
    '"latin1 ção ß ÿ direto"',
    '"CJK 中文 e emoji 😀 direto"',
    r'"escape no fim A"',
    r'"\u0000 nulo"',
  ];
  for (final f in fontes) {
    final s = jsonDecode(f) as String;
    print('  ${s.length} ${s.codeUnits.take(12).toList()} ${jsonEncode(s)}');
  }
  final longa = 'x' * 300 + 'ü' * 5;
  final d = jsonDecode(jsonEncode({'k': longa, longa: 1})) as Map;
  print('longa: ${d['k'] == longa} ${d[longa]} ${d.keys.last.length}');
  // Strings iguais decodificadas são iguais e têm o mesmo hash, e servem de chave.
  final a = jsonDecode('["abc", "abc", "\\u0061bc"]') as List;
  print('iguais: ${a[0] == a[1]} ${a[1] == a[2]} ${a[0].hashCode == a[2].hashCode} ${jsonDecode('""') == ''}');
}

void estrutura() {
  // Aninhamento profundo, alternando lista e objeto.
  var s = '0';
  for (var i = 0; i < 400; i++) {
    s = i.isEven ? '[$s, $i]' : '{"n$i": $s, "i": $i}';
  }
  Object? v = jsonDecode(s);
  var profundidade = 0;
  while (v is List || v is Map) {
    v = v is List ? v[0] : (v as Map).values.first;
    profundidade++;
  }
  print('profundidade: $profundidade $v');
  // Chaves repetidas: o último valor, na posição da primeira.
  final m = jsonDecode('{"a": 1, "b": 2, "a": 3, "c": {}, "b": [], "": null}') as Map;
  print('repetidas: $m ${m.keys.toList()} ${tipo(m)} ${tipo(m['b'])} ${tipo(m['c'])}');
  // Muitas chaves (o índice cresce) e listas grandes, com tipos certos.
  final grande = {for (var i = 0; i < 300; i++) 'chave$i': [i, '$i', i.isEven, null, i / 4]};
  final volta = jsonDecode(jsonEncode(grande)) as Map<String, dynamic>;
  print('grande: ${volta.length} ${volta['chave299']} ${jsonEncode(volta) == jsonEncode(grande)} ${tipo(volta['chave5'])}');
  // As listas devolvidas crescem e aceitam qualquer valor; os mapas também.
  final l = jsonDecode('[1, "a"]') as List;
  l.add({'x': 1});
  l.addAll([2, 3]);
  final o = jsonDecode('{"x": 1}') as Map<String, dynamic>;
  o['y'] = [1];
  o.remove('x');
  print('mutaveis: $l $o ${l.length}');
  print('vazios: ${jsonDecode('[]')} ${jsonDecode('{}')} ${jsonDecode('[[], {}, [{}]]')} ${jsonDecode(' 42 ')}');
}

void reviver() {
  final chamadas = <String>[];
  final r = jsonDecode('{"a": [1, 2, {"b": "x"}], "c": 3.5, "d": [[]]}', reviver: (k, v) {
    chamadas.add('$k:${v.runtimeType}');
    if (v is int) return v * 10;
    if (v is String) return v.toUpperCase();
    return v;
  });
  print('reviver: $r');
  print('chamadas: $chamadas');
}

void bytes() {
  final doc = {
    'nome': 'ação 中文 😀',
    'lista': [1, -2.5, true, null, 'x'],
    'aninhado': {'a': {'b': {'c': []}}},
  };
  final codec = json.fuse(utf8);
  final b = codec.encode(doc);
  print('bytes: ${b.length} ${codec.decode(b)}');
  // Em pedaços: o parser de UTF-8 continua entre um pedaço e outro.
  for (final tamanho in [1, 2, 3, 7]) {
    Object? resultado;
    final saida = ChunkedConversionSink<Object?>.withCallback((xs) => resultado = xs.single);
    final sink = utf8.decoder.fuse(const JsonDecoder()).startChunkedConversion(saida);
    for (var i = 0; i < b.length; i += tamanho) {
      sink.add(b.sublist(i, i + tamanho > b.length ? b.length : i + tamanho));
    }
    sink.close();
    print('  pedacos de $tamanho: ${jsonEncode(resultado) == jsonEncode(doc)}');
  }
  // Com BOM no começo.
  print('bom: ${codec.decode([0xEF, 0xBB, 0xBF, ...utf8.encode('{"x": [1]}')])}');
  // Texto em pedaços.
  Object? r2;
  final s2 = const JsonDecoder().startChunkedConversion(ChunkedConversionSink.withCallback((xs) => r2 = xs.single));
  const texto = '{"a": [1, 2, {"b": "c\\u00e9"}], "d": -12.5e1}';
  for (var i = 0; i < texto.length; i += 3) {
    s2.add(texto.substring(i, i + 3 > texto.length ? texto.length : i + 3));
  }
  s2.close();
  print('texto em pedacos: $r2');
}

void erros() {
  const ruins = [
    '[1, 2',
    '{"a" 1}',
    '{"a": 1,}',
    '[1,,2]',
    '"sem fim',
    '{"a": [}',
    'tru',
    '[01]',
    '[-]',
    '{1: 2}',
    '[1] x',
    '"\u0001"',
    '[1.e5]',
    '',
  ];
  for (final r in ruins) {
    try {
      print('  aceitou ${jsonDecode(r)}');
    } on FormatException catch (e) {
      print('  ${e.message} @${e.offset}');
    }
  }
  try {
    utf8.decoder.fuse(const JsonDecoder()).convert([0x5B, 0x31, 0x2C]);
  } on FormatException catch (e) {
    print('  utf8: ${e.message} @${e.offset}');
  }
  // Um decode que falha no meio não atrapalha o próximo.
  print('depois: ${jsonDecode('[{"ok": [true]}]')}');
}

void main() {
  numeros();
  textos();
  estrutura();
  reviver();
  bytes();
  erros();
}
