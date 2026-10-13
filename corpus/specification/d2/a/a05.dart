class C<T> {
  const C();
  m() {
    var a = const C<T>();
    var b = const <T>[];
    var c = const <T, int>{};
    var d = const <T>{};
    var e = const C<List<T>>();
    var f = const <List<T?>>[];
    return [a, b, c, d, e, f];
  }
}
