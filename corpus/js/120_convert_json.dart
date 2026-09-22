// dart:convert JSON: jsonEncode/jsonDecode de estruturas aninhadas, escapes, withIndent e toEncodable.
import 'dart:convert';

class Pessoa {
  final String nome;
  final int idade;
  Pessoa(this.nome, this.idade);
  Map<String, Object?> toJson() => {'nome': nome, 'idade': idade};
}

class SemToJson {
  final double valor;
  SemToJson(this.valor);
}

void main() {
  print(jsonEncode(1));
  print(jsonEncode(2.5));
  print(jsonEncode(true));
  print(jsonEncode(null));
  print(jsonEncode('texto'));
  print(jsonEncode('com "aspas" e \\ barra'));
  print(jsonEncode('quebra\nde linha\ttab'));
  print(jsonEncode('unicode é 😀'));
  print(jsonEncode('\u0001\u001f'));
  print(jsonEncode([]));
  print(jsonEncode({}));
  print(jsonEncode([1, 'a', null, true, 2.5]));
  print(jsonEncode({'a': 1, 'b': [1, 2], 'c': {'d': null}}));
  print(jsonEncode({'chave com "aspas"': 'v'}));
  print(jsonEncode([[1, [2, [3]]]]));
  print(jsonEncode({'n': -0.5, 'e': 1.5e-7}));
  print(jsonEncode(Pessoa('Ana', 30)));
  print(jsonEncode([Pessoa('A', 1), Pessoa('B', 2)]));
  print(jsonEncode({'p': Pessoa('C', 3)}));
  print(jsonEncode(SemToJson(2.5), toEncodable: (o) => {'valor': (o as SemToJson).valor}));
  print(jsonEncode([SemToJson(1.5)], toEncodable: (o) => 'custom'));
  print(jsonEncode(Duration(seconds: 5), toEncodable: (o) => (o as Duration).inSeconds));
  try {
    jsonEncode(SemToJson(1.5));
  } catch (e) {
    print('lançou ${e is JsonUnsupportedObjectError}');
  }

  var dec = jsonDecode('{"a": 1, "b": [true, null, "x"], "c": {"d": 2.5}}');
  print(dec);
  print(dec is Map);
  print(dec['a']);
  print(dec['a'] is int);
  print(dec['b'] is List);
  print(dec['b'][0]);
  print(dec['b'][1]);
  print(dec['b'][2]);
  print(dec['c']['d']);
  print(dec['c']['d'] is double);
  print((dec as Map<String, dynamic>).keys.toList());
  var lista = jsonDecode('[1, 2, 3]') as List;
  print(lista);
  print(lista.length);
  print((lista.cast<int>()).reduce((a, b) => a + b));
  print(jsonDecode('"só string"'));
  print(jsonDecode('42'));
  print(jsonDecode('-1.5'));
  print(jsonDecode('true'));
  print(jsonDecode('null'));
  print(jsonDecode('"escapado \\n \\t \\" \\\\ \\u00e9"'));
  print(jsonDecode('[]'));
  print(jsonDecode('{}'));
  print(jsonDecode(' [ 1 , 2 ] '));
  print(jsonDecode('{"a":{"b":{"c":[{"d":"fundo"}]}}}')['a']['b']['c'][0]['d']);
  try {
    jsonDecode('{invalido}');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  try {
    jsonDecode('');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }

  var enc = JsonEncoder.withIndent('  ');
  print(enc.convert({'a': 1, 'b': [1, 2], 'c': {}}));
  print(enc.convert([]));
  print(enc.convert([1, [2, 3]]));
  print(JsonEncoder.withIndent('\t').convert({'x': 'y'}));
  print(JsonEncoder().convert({'a': 1}));
  print(json.encode({'k': 'v'}));
  print(json.decode('[7]'));
  print(jsonDecode(jsonEncode({'round': ['trip', 1, 2.5, null]})));
  var rev = jsonDecode('{"z": 1, "a": 2, "m": 3}') as Map;
  print(rev.keys.toList());
  print(jsonDecode('{"dup": 1, "dup": 2}'));
  print(jsonDecode('[1.55e1]')[0]);
  print(jsonDecode('[-2.5e-1]')[0]);
  print(jsonDecode('"\\ud83d\\ude00"') == '😀');
  print(jsonEncode('\u2028\u2029'));
  print(jsonEncode('/'));
  print(jsonDecode('{"a": 1}', reviver: (k, v) => v is int ? v * 10 : v));
  print(jsonDecode('[1, 2]', reviver: (k, v) => k is int ? 'i$k' : v));
}
