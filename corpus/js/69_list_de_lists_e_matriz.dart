// listas de listas: matriz com List.generate, List.filled compartilha a mesma lista, transposição, flatten, cópias rasas/profundas.
List<List<int>> transposta(List<List<int>> m) =>
    List.generate(m[0].length, (j) => List.generate(m.length, (i) => m[i][j]));

String mostra(List<List<int>> m) => m.map((l) => l.join(' ')).join('\n');

void main() {
  final m = List.generate(3, (i) => List.generate(4, (j) => i * 4 + j));
  print(m);
  print(m.length);
  print(m[0].length);
  print(m[1][2]);
  print(mostra(m));

  // modificar uma célula não afeta outras linhas
  m[0][0] = 99;
  print(m[0]);
  print(m[1]);

  // List.filled com lista compartilha a MESMA instância
  final compartilhada = List.filled(3, <int>[]);
  compartilhada[0].add(1);
  print(compartilhada);
  print(identical(compartilhada[0], compartilhada[2]));
  compartilhada[2].add(2);
  print(compartilhada);

  // List.generate cria instâncias separadas
  final separada = List.generate(3, (_) => <int>[]);
  separada[0].add(1);
  print(separada);
  print(identical(separada[0], separada[1]));

  // transposição
  final t = transposta(m);
  print(t);
  print(t.length);
  print(t[0].length);
  print(mostra(transposta(t)) == mostra(m));

  // flatten via expand
  print(m.expand((l) => l).toList());
  print(m.expand((l) => l).length);
  print(t.expand((l) => l).where((x) => x.isEven).toList());

  // matriz irregular
  final irregular = [
    [1],
    [2, 3],
    [],
    [4, 5, 6]
  ];
  print(irregular);
  print(irregular.map((l) => l.length).toList());
  print(irregular.expand((l) => l).toList());
  print(irregular.where((l) => l.isNotEmpty).map((l) => l.last).toList());

  // toString aninhado com três níveis
  final cubo = List.generate(2, (i) => List.generate(2, (j) => List.generate(2, (k) => i + j + k)));
  print(cubo);
  print(cubo[1][1][1]);
  print(cubo.expand((p) => p).expand((l) => l).fold<int>(0, (a, b) => a + b));

  // cópia rasa: as linhas continuam compartilhadas
  final orig = [
    [1, 2],
    [3, 4]
  ];
  final rasa = List.of(orig);
  rasa[0][0] = 100;
  print(orig);
  rasa[1] = [0, 0];
  print(orig);
  print(rasa);
  final rasa2 = [...orig];
  print(identical(rasa2[0], orig[0]));
  final rasaFrom = List<List<int>>.from(orig);
  print(identical(rasaFrom[1], orig[1]));

  // cópia profunda via map(toList)
  final profunda = orig.map((l) => l.toList()).toList();
  profunda[0][0] = -1;
  print(orig);
  print(profunda);
  print(identical(profunda[0], orig[0]));
  final profunda2 = [for (final l in orig) [...l]];
  profunda2[1][1] = -4;
  print(orig);
  print(profunda2);

  // soma de linhas e colunas
  final grade = List.generate(3, (i) => List.generate(3, (j) => (i + 1) * (j + 1)));
  print(mostra(grade));
  print(grade.map((l) => l.fold<int>(0, (a, b) => a + b)).toList());
  print(List.generate(3, (j) => grade.fold<int>(0, (a, l) => a + l[j])));
  print(List.generate(3, (i) => grade[i][i]));

  // matriz de strings e busca 2D
  final tab = [
    ['a', 'b'],
    ['c', 'd']
  ];
  var pos = '';
  for (var i = 0; i < tab.length; i++) {
    for (var j = 0; j < tab[i].length; j++) {
      if (tab[i][j] == 'c') pos = '$i,$j';
    }
  }
  print(pos);
  print(tab.indexWhere((l) => l.contains('d')));

  // igualdade de linhas é identidade
  print(orig[0] == profunda[0]);
  print(orig[0] == orig[0]);

  // reversed em cada linha e na matriz
  print(grade.map((l) => l.reversed.toList()).toList());
  print(grade.reversed.toList());
  print(grade.reversed.map((l) => l.reversed.toList()).toList());
}
