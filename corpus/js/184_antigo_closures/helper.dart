// Fábrica original: cada chamada conserva estado independente depois do retorno.
int Function() makeCounter(int seed) {
  int current = seed;
  return () { current += 2; return current; };
}
int left(int value) { return value * 3; }
int right(int value) { return value * 3; }
int invoke(int Function(int) callback, int value) { return callback(value); }
int Function(int) selected() { return left; }
List<int> mapValues(List<int> values, int Function(int) callback) {
  return values.map(callback).toList();
}
