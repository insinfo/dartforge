void f(({int b, int a}) r1, (int, {String name}) r2, (int,) r3,
    (int, int)? r4, ({int a})? r5) {
  int a = r1;
  int b = r2;
  int c = r3;
  int d = r4;
  int e = r5;
  print([a, b, c, d, e]);
}
