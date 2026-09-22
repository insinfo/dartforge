// Convertido de tests/conformance/cases/collections_closures.dart (fixture antigo do corpus de conformidade).
// Programa original. Oracle: Dart VM 3.6.2; callbacks devem preservar efeitos e identidade.
int sameA(int n) { return n + 1; }
int sameB(int n) { return n + 1; }
int apply(int Function(int) callback, int value) { return callback(value); }
int Function() counter(int start) {
  int value = start;
  return () { value += 1; return value; };
}
int Function(int) choose() { return sameA; }
void main() {
  List<int> values = <int>[1, 2, 3, 4];
  values[1] = 5;
  values.add(6);
  print(values.length);
  print(values[1]);
  List<List<int>> matrix = <List<int>>[<int>[7, 8], <int>[9]];
  matrix[0][1] = 11;
  print(matrix[0][1]);
  var alias = values;
  alias[0] = 2;
  print(values[0]);
  int visits = 0;
  int transforms = 0;
  var pending = values.where((int value) { visits += 1; return value > 2; })
      .map((value) { transforms += 1; return value * 10; });
  print(visits);
  print(transforms);
  var materialized = pending.toList();
  print(visits);
  print(transforms);
  print(materialized.length);
  print(materialized[0]);
  print(pending.any((int value) => value >= 40));
  print(visits);
  print(transforms);
  int sum = 0;
  values.forEach((value) { sum += value; });
  print(sum);
  int mapCalls = 0;
  var lazyMap = values.map((int value) { mapCalls += 1; return value * 2; });
  print(lazyMap.length);
  print(mapCalls);
  print(lazyMap.isEmpty);
  print(mapCalls);
  print(lazyMap.last);
  print(mapCalls);
  var first = counter(10);
  var second = counter(20);
  print(first());
  print(first());
  print(second());
  print(first == second);
  var sameClosure = first;
  print(first == sameClosure);
  print(apply(choose(), 7));
  var topAlias = sameA;
  print(topAlias == sameA);
  print(sameA == sameB);
  List<int Function()> callbacks = <int Function()>[];
  for (var i = 0; i < 3; i++) { callbacks.add(() => i); }
  print(callbacks[0]());
  print(callbacks[1]());
  print(callbacks[2]());
  List<int Function()> captured = <int Function()>[];
  values.forEach((value) { captured.add(() => value); });
  print(captured[0]());
  print(captured[4]());
}
