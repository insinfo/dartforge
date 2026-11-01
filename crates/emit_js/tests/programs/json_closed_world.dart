// `jsonEncode` in a program where no class has a `toJson` member
// (`docs/JS-PRODUCAO-VELOCIDADE.md` §14): the default `toEncodable` is a
// plain JavaScript call that throws. Every answer is the VM's.
import 'dart:convert';

class Plain {
  final int x;
  Plain(this.x);
  @override
  String toString() => 'Plain($x)';
}

void main() {
  print(jsonEncode({
    'a': [1, 2.5, null, true, 'q"\\\n'],
    'b': {'c': <String, Object?>{}},
  }));
  for (final value in <Object?>[Plain(1), [Plain(2)], {'k': Plain(3)}, double.nan, #sym]) {
    try {
      print(jsonEncode(value));
    } on JsonUnsupportedObjectError catch (e) {
      print('unsupported ${e.unsupportedObject} ${e.cause is NoSuchMethodError} '
          '${e.partialResult}');
    }
  }
  // A `toEncodable` of its own still runs.
  print(jsonEncode([Plain(4)], toEncodable: (o) => (o as Plain).x));
  print(const JsonEncoder().convert({'n': -0.5}));
  print(json.decode('{"x": [1, {"y": null}]}'));
}
