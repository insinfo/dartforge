import 'dart:math' as p;
void g(Object? o) {}
void f() {
  p;
  p();
  p = 1;
  p += 1;
  p++;
  p ??= 1;
  p?.pi;
  p?.max(1, 2);
  p[0];
  g(p);
  p..toString();
  p.pi;
  p.max(1, 2);
  p == p;
  for (p in []) {}
}
