// Argumentos de tipo deduzidos pelo supertipo do argumento real (`Iterable<T>`
// contra `List<int>`) e `X?` casado com `T?` dando `X = T`: o `nonNulls` de
// uma `List<T?>` no `popTypedList` do `ast_builder` do analyzer.
import 'dart:collection';

List<T> copia<T>(Iterable<T> it) => <T>[...it];

List<T>? semNulos<T extends Object>(int n, List<T?> fonte) {
  var cauda = List<T?>.filled(n, null, growable: true);
  for (var i = 0; i < n; i++) cauda[i] = fonte[i];
  return cauda.nonNulls.toList();
}

Set<String> conjunto(Iterable<Object> xs) => HashSet.of(xs.map((x) => '$x'));

void main() {
  print(copia([1, 2]).runtimeType);
  print(copia(<String>{'a'}).runtimeType);
  print(semNulos<String>(2, ['a', null]).runtimeType);
  print(conjunto([1, 2]).runtimeType);
  final s = HashSet.of(<int>[1]);
  print(s.runtimeType);
  print(<int>[1, 2].map((x) => '$x').toList().runtimeType);
}
