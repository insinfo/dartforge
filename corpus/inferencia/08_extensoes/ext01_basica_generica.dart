// R-EXT-01: extensão aplicável: os argumentos de tipo são inferidos do
// receptor.
extension on String {
  int get dobro => length * 2;
}

extension E<T> on List<T> {
  T get primeiro => first;
  List<R> mapa<R>(R Function(T) f) => [for (var e in this) f(e)];
}

extension N<T extends num> on Iterable<T> {
  T soma() => reduce((a, b) => (a + b) as T);
}

void main() {
  print([/*@*/'a'.dobro, /*@*/[1].primeiro, /*@*/[1.5].soma(), /*@*/[1].mapa((x) => '$x')]);
}
