void f(Xyz p, List<Xyz> q) {
  int a = p;
  String b = p.foo;
  p.bar(1, 2);
  int c = q.first;
  q.add(3);
  p + 1;
  -p;
  p[0];
  p();
  for (var e in p) { e.x; }
  if (p) {}
  int d = p as int;
  print(p is String);
  print([a, b, c, d]);
}
