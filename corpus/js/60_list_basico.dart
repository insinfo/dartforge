// List básico: literal, add/insert/remove*, []/[]=, length, first/last/single, indexOf, sublist, reversed, clear.
void main() {
  final l = [3, 1, 2];
  print(l);
  print(l.length);
  print(l[0]);
  l[0] = 30;
  print(l);
  print(l.first);
  print(l.last);
  print(l.isEmpty);
  print(l.isNotEmpty);

  l.add(4);
  l.addAll([5, 6]);
  print(l);
  l.insert(0, 0);
  print(l);
  l.insertAll(2, [-1, -2]);
  print(l);

  print(l.remove(-1));
  print(l.remove(99));
  print(l);
  print(l.removeAt(1));
  print(l);
  print(l.removeLast());
  print(l);
  l.removeWhere((x) => x < 0);
  print(l);
  l.retainWhere((x) => x != 1);
  print(l);

  // encolher via length
  l.length = 2;
  print(l);
  l.addAll([7, 8, 9, 7]);
  print(l);

  print(l.indexOf(7));
  print(l.lastIndexOf(7));
  print(l.indexOf(100));
  print(l.contains(8));
  print(l.contains(100));

  print(l.sublist(1));
  print(l.sublist(1, 3));
  print(l.sublist(2, 2));
  print(l.reversed.toList());
  print(l.reversed.first);

  // single
  print([42].single);
  try {
    [1, 2].single;
  } catch (e) {
    print('single em lista com 2: ${e is StateError}');
  }
  try {
    <int>[].first;
  } catch (e) {
    print('first em vazia: ${e is StateError}');
  }

  // firstOrNull não existe no core; usar isEmpty
  final vazia = <String>[];
  print(vazia.isEmpty ? 'sem elementos' : vazia.first);

  // toString aninhado e com strings/null
  print(['a', null, 1, 2.5, true]);
  print([
    [1, 2],
    [3]
  ]);
  print([[]]);
  print(<int>[]);

  // igualdade é de identidade
  final a = [1, 2];
  final b = [1, 2];
  print(a == b);
  print(a == a);
  print(identical(a, b));

  // remove com objeto igual (==) e não idêntico
  final strs = ['x', 'y', 'z'];
  print(strs.remove('y'));
  print(strs);

  // clear
  strs.clear();
  print(strs);
  print(strs.length);

  // índice negativo ou fora do intervalo
  try {
    print(l[10]);
  } catch (e) {
    print('fora do intervalo: ${e is RangeError}');
  }
  try {
    l[-1] = 0;
  } catch (e) {
    print('índice negativo: ${e is RangeError}');
  }

  // lista com tipos diversos e growable padrão
  final mista = <Object>[1, 'dois', [3]];
  mista.add(4.5);
  print(mista);
  print(mista.length);

  // += com lista e operador + cria nova
  var c = [1];
  final antes = c;
  c += [2, 3];
  print(c);
  print(identical(antes, c));
  print(antes);
}
