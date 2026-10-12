// JSON e o que o `jsonEncode`/`jsonDecode` do nativo passa a fazer em linha
// ou no runtime (docs/NATIVO-PLANO.md, seção JSON): `String.codeUnitAt` e
// `length` lidos em linha (de parâmetro, de local e de campo relido, com o
// cache do ponto de leitura), o `Double_parse` do `_parseDouble` do
// `convert_patch.dart` (o `double` sem caixa), o `int.toString` e o
// `double.toString` pelo runtime (com o cache de 8 entradas da VM), e o
// `StringBuffer.write`/`writeCharCode` direto no acumulador. A saída tem de
// ser a da VM.
import 'dart:convert';

String erro(void Function() f) {
  try {
    f();
    return 'sem erro';
  } catch (e) {
    return '${e.runtimeType}: $e';
  }
}

class Leitor {
  String texto;
  Leitor(this.texto);

  // O campo relido a cada volta: o cache do ponto de leitura.
  int somar() {
    var h = 0;
    for (var i = 0; i < texto.length; i++) {
      h = (h * 31 + texto.codeUnitAt(i)) & 0xFFFFFF;
    }
    return h;
  }

  // O campo trocado no meio do laço: o cache tem de ver a string nova.
  int trocarNoMeio(String outro) {
    var h = 0;
    for (var i = 0; i < 6; i++) {
      if (i == 3) texto = outro;
      h = h * 1000 + texto.codeUnitAt(i % texto.length);
    }
    return h;
  }

  int unidade(int i) => texto.codeUnitAt(i);
}

int somaParam(String s) {
  var h = s.length;
  for (var i = 0; i < s.length; i++) {
    h = (h * 31 + s.codeUnitAt(i)) & 0xFFFFFF;
  }
  return h;
}

void main() {
  // codeUnitAt e length: Latin-1, duas unidades, pares, vazia, índices
  // fora da faixa (o RangeError da VM).
  for (final s in ['', 'a', 'ação', 'CJK 中文', '😀x𝄞', 'x' * 300]) {
    final l = Leitor(s);
    print('${s.length} ${somaParam(s)} ${l.somar()}');
  }
  final l = Leitor('abcdef');
  print(l.trocarNoMeio('XYZ😀'));
  print(l.unidade(0));
  print(erro(() => l.unidade(-1)));
  print(erro(() => l.unidade(5)));
  print(erro(() => ''.codeUnitAt(0)));
  print(erro(() => 'ab'.codeUnitAt(2)));
  print(erro(() => '😀'.codeUnitAt(2)));
  final dyn = <Object?>['xyz', 'ação', ''];
  for (final v in dyn) {
    print((v as dynamic).length);
  }

  // Números no JSON: 17 algarismos (o `_parseDouble`, não o caminho
  // rápido), expoentes, -0.0, inteiros grandes.
  const numeros = '[2.1200000000000002e-7, 0.30000000000000004, 1.5e21, -14285.714285714286, '
      '1e-9, 9.99e+23, -0.0, 0.0, 4611686018427387903, -9007199254740993, 123.456, 1E3, 5e-324, '
      '1.7976931348623157e308, 12345678901234567890]';
  final lidos = jsonDecode(numeros) as List;
  for (final n in lidos) {
    print('${n.runtimeType} $n');
  }
  print(jsonEncode(lidos));
  print(erro(() => jsonDecode('[1.5e]')));
  print(erro(() => jsonDecode('[-]')));

  // int.toString: a tabela pequena da VM (a mesma string), negativos,
  // os extremos.
  for (final i in [0, 7, -7, 99, -99, 100, -100, 12345, -12345, 9223372036854775807, -9223372036854775808]) {
    print('${i.toString()} ${i.toString().length}');
  }
  print(identical(5.toString(), 5.toString()));
  print(identical((-42).toString(), (-42).toString()));

  // double.toString: o cache de 8 entradas da VM devolve a mesma string
  // para o mesmo `double` (e não para -0.0 contra 0.0).
  final d = 1 / 3;
  final a = d.toString();
  print(identical(a, d.toString()));
  print(identical((0.0).toString(), (0.0).toString()));
  print('${(-0.0).toString()} ${(0.0).toString()} ${double.nan} ${double.infinity} ${-double.infinity}');
  for (var i = 0; i < 20; i++) {
    (i + 0.5).toString();
  }
  print(identical(a, d.toString()));
  final vistos = <String>[];
  for (final x in [0.1, 0.2, 0.1 + 0.2, 1e21, 1e-7, 123.0, -0.5, 2.5e-5]) {
    vistos.add(x.toString());
  }
  print(vistos.join(' '));

  // StringBuffer: write de String, de outros objetos, de vazio;
  // writeCharCode de Latin-1, duas unidades, par substituto e fora da
  // faixa (o RangeError do writeCharCode da VM).
  final sb = StringBuffer('início ');
  sb.write('ação');
  sb.write(42);
  sb.write(null);
  sb.write('');
  sb.write(2.5);
  sb.writeCharCode(0x41);
  sb.writeCharCode(0x4E2D);
  sb.writeCharCode(0x1F600);
  sb.writeCharCode(0);
  sb.writeAll(['x', 1, true], ',');
  sb.writeln('fim');
  print('${sb.length} ${sb.toString().length} ${somaParam(sb.toString())}');
  print(sb.toString().replaceAll('\u0000', '<0>'));
  print(erro(() => sb.writeCharCode(-1)));
  print(erro(() => sb.writeCharCode(0x110000)));
  print(sb.length);

  // Ida e volta com escapes, Unicode e surrogates soltos.
  final doc = {
    'aspas': 'a"b\\c/d',
    'controle': '\n\t\r\b\f\u0001\u001f',
    'unicode': 'ação 中文 😀 𝄞',
    'solto': 'x\uD800y\uDC00z',
    'numeros': [1, -2, 3.5, 1e21, 0.1 + 0.2, -0.0],
    'aninhado': {
      'vazio': {},
      'lista': [[], [null, true, false]],
    },
  };
  final texto = jsonEncode(doc);
  print(texto);
  print(somaParam(texto));
  final volta = jsonDecode(texto);
  print(jsonEncode(volta) == texto);
  print(const JsonEncoder.withIndent('  ').convert(doc));
  final bytes = json.fuse(utf8).encode(doc);
  print('${bytes.length} ${json.fuse(utf8).decode(bytes) is Map}');
  print(jsonDecode(texto, reviver: (k, v) => v is double ? v.round() : v));
  print(erro(() => jsonDecode('{"a": "x\u0001"}')));
  print(erro(() => jsonDecode('"abc')));
}
