void f(bool c, Object o) {
  while (c) var x = 0;
  if (c) var y = 1; else var z = 2;
  for (var (p) = 0; c;) {}
  for (var (q, r) = (0, 1); q < 1;) {}
  int w;
  [w, final v] = [1, 2];
}
