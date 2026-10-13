// Regression: with a lot of live data near the heap ceiling, the collection
// trigger has hysteresis (before, going past half the ceiling collected on
// every allocation and the program stopped making progress); and the array of
// a list literal inside a loop is created in the entry block (before, one
// `alloca` per iteration overflowed the stack).
void main() {
  // ~ many live objects (above half the ceiling) and temporaries around them.
  final vivos = <List<int>>[];
  for (var i = 0; i < 9000; i++) {
    vivos.add(List<int>.filled(400, i));
  }
  var soma = 0;
  for (var i = 0; i < 300000; i++) {
    final t = <int>[i, i + 1, i + 2];
    soma += t[1];
  }
  print('${vivos.length} $soma');
}
