void f<T>(Iterable<T> Function() g, int Function(T) h) {
  [for (var x in g()) if (x is String) h(x)];
}
