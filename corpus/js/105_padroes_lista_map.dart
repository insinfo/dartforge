// Padrões de lista (... no início/meio/fim, vazias, aninhadas, tipadas) e de mapa (chaves ausentes não casam).
String lista(List<Object?> xs) => switch (xs) {
      [] => 'vazia',
      [var a] => 'um: $a',
      [var a, var b] => 'dois: $a $b',
      [var a, ..., var z] when xs.length == 3 => 'tres: $a .. $z',
      [var a, var b, ...var resto] => 'muitos: $a $b resto=$resto',
    };

String inicio(List<int> xs) => switch (xs) {
      [..., 9] => 'termina em 9',
      [1, ...] => 'comeca em 1',
      [_, 5, ..., 8] => 'segundo 5 ultimo 8',
      [..., 7, _] => 'penultimo 7',
      _ => 'nada',
    };

String aninhada(List<List<int>> m) => switch (m) {
      [[]] => 'uma linha vazia',
      [[var a], [var b]] => 'diagonal $a,$b',
      [[var a, ...], [var b, ...], ...] => 'primeiras colunas $a,$b',
      _ => 'outra',
    };

String tipada(Object o) => switch (o) {
      <int>[var a, var b] => 'ints $a+$b=${a + b}',
      <String>[var a, ...] => 'strings comecando ${a.toUpperCase()}',
      [int a, String b] => 'int e string $a $b',
      List<int>() => 'lista de int generica',
      List() => 'lista qualquer',
      _ => 'nao lista',
    };

String mapa(Map<String, Object?> m) => switch (m) {
      {'tipo': 'a', 'v': int v} => 'a com $v',
      {'tipo': 'a'} => 'a sem v',
      {'tipo': String t, 'v': var v} when v != null => 'tipo $t v=$v',
      {'tipo': var t} => 'tipo $t (v nulo ou ausente)',
      {'x': _, 'y': _} => 'coordenada',
      Map() => 'qualquer mapa',
    };

String soChaves(Map<int, String> m) => switch (m) {
      {1: 'um', 2: 'dois'} => 'um e dois',
      {1: var u} => 'so um: $u',
      _ => 'sem um',
    };

void main() {
  print(lista([]));
  print(lista([1]));
  print(lista(['a', null]));
  print(lista([1, 2, 3]));
  print(lista([1, 2, 3, 4, 5]));

  print(inicio([2, 9]));
  print(inicio([1, 2, 3]));
  print(inicio([0, 5, 6, 8]));
  print(inicio([0, 0, 7, 0]));
  print(inicio([3, 4]));
  print(inicio([]));
  print(inicio([9]));
  print(inicio([1]));

  print(aninhada([[]]));
  print(aninhada([[1], [2]]));
  print(aninhada([[1, 2], [3, 4], [5, 6]]));
  print(aninhada([[1, 2]]));
  print(aninhada([]));

  print(tipada(<int>[1, 2]));
  print(tipada(<int>[1, 2, 3]));
  print(tipada(<String>['x', 'y']));
  print(tipada(<Object>[1, 'b']));
  print(tipada(<num>[1, 2]));
  print(tipada(<Object>[1, 2]));
  print(tipada('nao'));
  print(tipada(<int>[]));

  print(mapa({'tipo': 'a', 'v': 5}));
  print(mapa({'tipo': 'a', 'v': 'nao int'}));
  print(mapa({'tipo': 'a'}));
  print(mapa({'tipo': 'b', 'v': 'x'}));
  print(mapa({'tipo': 'b', 'v': null}));
  print(mapa({'tipo': 'b'}));
  print(mapa({'x': 1, 'y': 2}));
  print(mapa({'x': 1}));
  print(mapa({}));
  print(mapa({'tipo': 'b', 'v': 1, 'extra': 2}));

  print(soChaves({1: 'um', 2: 'dois'}));
  print(soChaves({1: 'um', 2: 'DOIS'}));
  print(soChaves({2: 'dois', 1: 'um'}));
  print(soChaves({1: 'x'}));
  print(soChaves({2: 'dois'}));
  print(soChaves({}));

  final registros = [
    {'nome': 'ana', 'tags': ['a', 'b']},
    {'nome': 'bia', 'tags': <String>[]},
    {'nome': 'caio'},
  ];
  for (final r in registros) {
    final desc = switch (r) {
      {'nome': String n, 'tags': [var primeira, ...]} => '$n tag $primeira',
      {'nome': String n, 'tags': []} => '$n sem tags',
      {'nome': String n} => '$n sem campo tags',
      _ => '?',
    };
    print(desc);
  }
}
