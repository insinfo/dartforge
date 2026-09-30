// JSON (`dart:convert`): decodificação e codificação de documentos realistas
// — pequeno (~1 KB, muitas vezes), médio (~100 KB) e grande (~6 MB) — com
// objetos aninhados, listas de `int` e `double`, strings com escapes e
// Unicode (inclusive fora do BMP), `JsonEncoder.withIndent`,
// `json.fuse(utf8)` (bytes) e `jsonDecode` com reviver.
//
// Os documentos saem de um gerador determinístico; o resultado de cada núcleo
// é uma soma de conferência barata da estrutura ou do texto, e `main` imprime
// antes das medidas a soma completa (todas as unidades) de cada documento
// codificado e a conferência da ida e volta, para comparar com a VM.
import 'dart:convert';

import 'comum.dart';

/// Gerador congruencial determinístico (os mesmos números em todo executor).
class Gerador {
  int _s;
  Gerador(this._s);
  int proximo() => _s = (_s * 1103515245 + 12345) & 0x7fffffff;
  int ate(int n) => proximo() % n;
}

const _palavras = [
  'alfa', 'beta', 'gama', 'delta', 'epsilon', 'zeta', 'eta', 'teta', //
  'iota', 'capa', 'lambda', 'mi', 'ni', 'csi', 'omicron', 'pi',
];

// Strings com o que o codificador tem de escapar e com Unicode de 2, 3 e 4
// bytes em UTF-8 (os dois últimos com pares substitutos).
const _especiais = [
  'aspas "duplas" e barra \\ invertida',
  'linha\nnova\ttab\rretorno',
  'controle \u0001\u0008\u000c\u001f fim',
  'acentuação: ção, ã, é, ü, ß',
  'CJK 中文字符 日本語 한국어',
  'emoji 😀🎉 e clave 𝄞 fora do BMP',
  'barra / normal e <tags> & entidades',
  '',
];

String _texto(Gerador g) {
  final n = 1 + g.ate(6);
  final sb = StringBuffer();
  for (var i = 0; i < n; i++) {
    if (i > 0) sb.write(' ');
    sb.write(_palavras[g.ate(_palavras.length)]);
  }
  if (g.ate(4) == 0) sb.write(_especiais[g.ate(_especiais.length)]);
  return sb.toString();
}

double _real(Gerador g) {
  switch (g.ate(5)) {
    case 0:
      return g.ate(1000000) / 1000; // 123.456
    case 1:
      return -g.ate(100000) / 7; // dízimas
    case 2:
      return g.ate(1000) * 1e-9; // expoente negativo
    case 3:
      return g.ate(1000) * 1.5e21; // expoente positivo
    default:
      return g.ate(100).toDouble(); // 42.0
  }
}

int _inteiro(Gerador g) {
  switch (g.ate(4)) {
    case 0:
      return g.ate(100);
    case 1:
      return -g.ate(1000000);
    case 2:
      return g.proximo() * g.proximo(); // até ~2^62
    default:
      return g.ate(1 << 20);
  }
}

Map<String, dynamic> _registro(Gerador g, int id) => {
      'id': id,
      'nome': _texto(g),
      'ativo': g.ate(2) == 0,
      'pontuacao': _real(g),
      'nulo': null,
      'tags': [for (var i = 0; i < g.ate(5); i++) _palavras[g.ate(16)]],
      'valores': [for (var i = 0; i < 8; i++) _inteiro(g)],
      'medidas': [for (var i = 0; i < 6; i++) _real(g)],
      'endereco': {
        'rua': _texto(g),
        'numero': g.ate(5000),
        'cidade': _especiais[g.ate(_especiais.length)],
        'geo': {'lat': _real(g), 'lng': _real(g)},
      },
      'historico': [
        for (var i = 0; i < g.ate(3); i++)
          {'quando': 1700000000 + g.ate(1000000), 'nota': _texto(g), 'ok': g.ate(3) != 0},
      ],
    };

/// Documento com `n` registros e um cabeçalho.
Map<String, dynamic> documento(int semente, int n) {
  final g = Gerador(semente);
  return {
    'versao': 3,
    'gerado': 'dartforge “bench” ✓',
    'registros': [for (var i = 0; i < n; i++) _registro(g, i)],
    'resumo': {'total': n, 'fator': 0.1 + 0.2, 'vazio': {}, 'lista_vazia': []},
  };
}

/// Soma de conferência de todas as unidades de um texto.
int somaTexto(String s) {
  var h = s.length;
  for (var i = 0; i < s.length; i++) {
    h = (h * 31 + s.codeUnitAt(i)) & 0x3FFFFFFF;
  }
  return h;
}

/// Soma de conferência barata de um valor decodificado: percorre a
/// estrutura inteira, mas de cada string só o comprimento e a primeira
/// unidade, e de cada `double` a parte inteira de mil vezes o valor limitada.
int somaValor(Object? v) {
  if (v == null) return 7;
  if (v is bool) return v ? 11 : 13;
  if (v is int) return v & 0xFFFFFF;
  if (v is double) return (v * 1000).clamp(-1e9, 1e9).toInt() & 0xFFFFFF;
  if (v is String) return v.isEmpty ? 17 : v.length * 131 + v.codeUnitAt(0);
  if (v is List) {
    var h = 19 + v.length;
    for (final e in v) {
      h = (h * 31 + somaValor(e)) & 0x3FFFFFFF;
    }
    return h;
  }
  if (v is Map) {
    var h = 23 + v.length;
    v.forEach((k, e) {
      h = (h * 31 + somaValor(k) + somaValor(e)) & 0x3FFFFFFF;
    });
    return h;
  }
  throw StateError('tipo inesperado ${v.runtimeType}');
}

/// Conferência barata do texto codificado: o comprimento e uma unidade a
/// cada 97.
int amostra(String s) {
  var h = s.length;
  for (var i = 0; i < s.length; i += 97) {
    h = (h * 31 + s.codeUnitAt(i)) & 0x3FFFFFFF;
  }
  return h;
}

void main() {
  final pequenoObj = documento(1, 2);
  final medioObj = documento(2, 200);
  final grandeObj = documento(3, 12000);
  final pequeno = jsonEncode(pequenoObj);
  final medio = jsonEncode(medioObj);
  final grande = jsonEncode(grandeObj);
  final bytesMedio = utf8.encode(medio);
  final indentado = const JsonEncoder.withIndent('  ').convert(medioObj);

  // Conferência completa, fora do tempo: tamanhos, soma de todas as
  // unidades e a ida e volta (decodificar e codificar de novo dá o mesmo).
  for (final (nome, s) in [('pequeno', pequeno), ('medio', medio), ('grande', grande), ('indentado', indentado)]) {
    print('$nome: ${s.length} unidades, soma ${somaTexto(s)}, '
        'ida e volta ${jsonEncode(jsonDecode(s)) == (nome == 'indentado' ? medio : s)}');
  }
  print('bytes: ${bytesMedio.length}, ${json.fuse(utf8).decode(bytesMedio) is Map}');
  print('decodificado: ${somaValor(jsonDecode(grande))}');

  medir('decode_pequeno', () {
    var h = 0;
    for (var i = 0; i < 2000; i++) {
      h = (h + somaValor(jsonDecode(pequeno))) & 0x3FFFFFFF;
    }
    return h;
  });
  medir('encode_pequeno', () {
    var h = 0;
    for (var i = 0; i < 2000; i++) {
      h = (h + amostra(jsonEncode(pequenoObj))) & 0x3FFFFFFF;
    }
    return h;
  });
  medir('decode_medio', () {
    var h = 0;
    for (var i = 0; i < 30; i++) {
      h = (h + somaValor(jsonDecode(medio))) & 0x3FFFFFFF;
    }
    return h;
  });
  medir('encode_medio', () {
    var h = 0;
    for (var i = 0; i < 30; i++) {
      h = (h + amostra(jsonEncode(medioObj))) & 0x3FFFFFFF;
    }
    return h;
  });
  medir('decode_grande', () => somaValor(jsonDecode(grande)), rodadas: 4);
  medir('encode_grande', () => amostra(jsonEncode(grandeObj)), rodadas: 4);
  medir('indentado', () {
    var h = 0;
    for (var i = 0; i < 20; i++) {
      h = (h + amostra(const JsonEncoder.withIndent('  ').convert(medioObj))) & 0x3FFFFFFF;
    }
    return h;
  });
  final codec = json.fuse(utf8);
  medir('utf8_bytes', () {
    var h = 0;
    for (var i = 0; i < 20; i++) {
      final b = codec.encode(medioObj);
      h = (h + b.length + somaValor(codec.decode(b))) & 0x3FFFFFFF;
    }
    return h;
  });
  medir('reviver', () {
    var h = 0;
    for (var i = 0; i < 20; i++) {
      final v = jsonDecode(medio, reviver: (k, v) {
        if (v is double) return v.round();
        if (k == 'nome' && v is String) return v.toUpperCase();
        return v;
      });
      h = (h + somaValor(v)) & 0x3FFFFFFF;
    }
    return h;
  });
}
