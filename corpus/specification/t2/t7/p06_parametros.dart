class K {
  void m(int p, {covariant int z = 0, required String a}) {}
  void n(int p, [String? q = 'x', int r = 1]) {}
}

void f(K k, void Function({int zz, required int aa, int? mm}) g) {
  int x = k.m;
  int y = k.n;
  int w = g;
  print([x, y, w]);
}
