// Rótulos homônimos aninhados (o interno sombreia o externo, também através
// de `finally`); `for (alvo in e)` com campo, variável global ou setter como
// alvo; o corpo de `if`/`else`/laço sem bloco e o corpo de um `case` têm
// escopo próprio; valor padrão de função local que nomeia uma `const` local;
// padrões `const (…)` canônicos; e este `main` chega pelo `export` de main.dart.
import 'dart:collection';

var top = -1;
final trace = <String>[];
set traced(int v) => trace.add('set $v');

class Box {
  var field = -1;
  int sum() {
    var count = 0;
    for (field in {1, 2}) {
      count += field;
    }
    return count;
  }
}

class C {
  final int x;
  const C(this.x);
}

int labels() {
  var i = 0;
  try {
    L:
    while (i <= 0) {
      try {
        i = 1;
        L:
        while (true) {
          i += 10;
          if (i > 30) break L;
        }
        i += 1000;
      } finally {
        i += 100;
      }
    }
  } finally {
    i += 10000;
  }
  return i;
}

String scopes(bool c) {
  var a = 'outer';
  if (c)
    var a = 'then';
  else
    var a = 'else';
  for (var i = 0; i < 2; i++) var a = 'for';
  while (a == 'never') var a = 'while', b = 'b';
  do var a = 'do'; while (a == 'never');
  switch (a) {
    case var x:
      var x = 'local';
      a = '$a $x';
  }
  if (a case var y) var y = 'shadow';
  return a;
}

void main() {
  print(labels());
  print(Box().sum());
  for (top in [1, 2]) {}
  print(top);
  for (traced in [3, 4]) {}
  print(trace);
  print(scopes(true));
  print(scopes(false));

  const missing = 'missing';
  String f([String p = missing]) => p;
  print(f());
  print(f('given'));

  for (final v in [const C(42), C(42), <int>[42], const <int>[42]]) {
    switch (v) {
      case const (C(42)):
        print('constant C');
      case const (<int>[42]):
        print('constant list');
      default:
        print('other ${v.runtimeType}');
    }
  }
  print(SplayTreeSet.of([3, 1, 2]));
}
