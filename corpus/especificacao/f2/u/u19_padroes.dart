void f(Object o, (int, int) r) {
  var (a, b) = r;
  var (c, d) = r;
  var [e, _] = [1, 2];
  final (g, h) = r;
  print(c);
  if (o case int x) {}
  if (o case [int y, int z]) { print(y); }
  switch (o) {
    case int p || [int p]:
      break;
    case (int q, int w) || [int q, int w]:
      print(q);
  }
  switch (o) {
    case int m when m > 0:
    case [int m]:
      break;
  }
  var s = switch (o) { int n => 1, String t => t.length, _ => 0 };
  print(s);
  int u, v;
  (u, v) = r;
}
