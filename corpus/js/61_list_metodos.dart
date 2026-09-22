// métodos de List: sort, join, take/skip, *Where, any/every, asMap, ranges, generate/filled/of/unmodifiable, +.
void main() {
  final l = [5, 3, 9, 1, 7];
  l.sort();
  print(l);
  l.sort((a, b) => b.compareTo(a));
  print(l);
  final palavras = ['pera', 'uva', 'abacaxi', 'kiwi'];
  palavras.sort((a, b) => a.length.compareTo(b.length));
  print(palavras);
  palavras.sort();
  print(palavras);

  print(l.join());
  print(l.join(', '));
  print(<int>[].join('-'));
  print([1].join('-'));

  print(l.take(2).toList());
  print(l.skip(3).toList());
  print(l.take(100).toList());
  print(l.skip(100).toList());
  print(l.takeWhile((x) => x > 4).toList());
  print(l.skipWhile((x) => x > 4).toList());

  print(l.firstWhere((x) => x < 5));
  print(l.lastWhere((x) => x > 5));
  print(l.singleWhere((x) => x == 3));
  print(l.firstWhere((x) => x > 100, orElse: () => -1));
  print(l.lastWhere((x) => x > 100, orElse: () => -2));
  print(l.singleWhere((x) => x > 100, orElse: () => -3));
  try {
    l.firstWhere((x) => x > 100);
  } catch (e) {
    print('firstWhere sem orElse: ${e is StateError}');
  }
  try {
    l.singleWhere((x) => x > 2);
  } catch (e) {
    print('singleWhere com vários: ${e is StateError}');
  }

  print(l.any((x) => x > 8));
  print(l.any((x) => x > 9));
  print(l.every((x) => x > 0));
  print(l.every((x) => x > 1));
  print(<int>[].any((x) => true));
  print(<int>[].every((x) => false));

  print(l.indexWhere((x) => x < 5));
  print(l.indexWhere((x) => x < 5, 4));
  print(l.lastIndexWhere((x) => x > 5));
  print(l.indexWhere((x) => x > 100));

  print(l.asMap());
  print(l.asMap()[2]);
  print(l.asMap().keys.toList());

  final r = [0, 1, 2, 3, 4, 5, 6];
  print(r.getRange(2, 5).toList());
  r.setRange(0, 2, [10, 11]);
  print(r);
  r.setRange(3, 5, [100, 101, 102, 103], 1);
  print(r);
  r.fillRange(4, 7, 0);
  print(r);
  r.replaceRange(0, 2, [7, 8, 9]);
  print(r);
  r.replaceRange(0, 3, []);
  print(r);
  r.setAll(1, [-1, -2]);
  print(r);

  print(List.generate(4, (i) => i * i));
  print(List.generate(0, (i) => i));
  print(List.generate(3, (i) => 'x' * (i + 1)));

  final fixa = List.filled(3, 'a');
  print(fixa);
  fixa[1] = 'b';
  print(fixa);
  try {
    fixa.add('c');
  } catch (e) {
    print('filled não cresce: ${e is UnsupportedError}');
  }
  try {
    fixa.removeLast();
  } catch (e) {
    print('filled não encolhe: ${e is UnsupportedError}');
  }
  final cresce = List.filled(2, 0, growable: true);
  cresce.add(1);
  print(cresce);

  final copia = List.of(l);
  copia.add(0);
  print(copia);
  print(l);
  final copiaFrom = List<num>.from(l);
  copiaFrom.add(2.5);
  print(copiaFrom);

  final imutavel = List.unmodifiable([1, 2, 3]);
  print(imutavel);
  print(imutavel[1]);
  try {
    imutavel[0] = 9;
  } catch (e) {
    print('unmodifiable []=: ${e is UnsupportedError}');
  }
  try {
    imutavel.add(4);
  } catch (e) {
    print('unmodifiable add: ${e is UnsupportedError}');
  }
  try {
    imutavel.sort();
  } catch (e) {
    print('unmodifiable sort: ${e is UnsupportedError}');
  }
  try {
    imutavel.clear();
  } catch (e) {
    print('unmodifiable clear: ${e is UnsupportedError}');
  }

  print([1, 2] + [3]);
  print(<int>[] + []);
  final soma = [1] + [2] + [3];
  print(soma);

  // sort estável com comparator por chave
  final pessoas = [('ana', 30), ('bia', 25), ('caio', 30), ('dani', 25)];
  pessoas.sort((a, b) => a.$2.compareTo(b.$2));
  print(pessoas.map((p) => p.$1).toList());

  // shuffle não é determinístico; sort de strings com acentos por codeUnit
  final acent = ['é', 'e', 'z', 'a'];
  acent.sort();
  print(acent);
}
