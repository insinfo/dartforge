class C<T> {
  m() {
    var a = const <T Function(int)>[];
    var b = const <int Function(T)>[];
    var c = const <(T, int)>[];
    var d = const <({T? x})>{};
    var e = const <List<T>, Map<int, T?>>{};
    var f = const <void Function<X>(X)>[];
    var g = const <X Function<X extends T>()>[];
    const h = [<T>[]];
    var i = const [<T>{}];
    return [a, b, c, d, e, f, g, h, i];
  }
  void n({List<T> p = const <T>[], Map<T, int> q = const <T, int>{}, Object r = <T>[]}) {}
}
