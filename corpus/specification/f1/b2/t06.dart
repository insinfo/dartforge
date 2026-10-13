import 'dart:math' as p;
p.Undef? v;
void f(Object o) {
  new p.Undef();
  const p.Undef();
  new p.Undef.named();
  new p.pi();
  try {} on p.Undef catch (e) {}
  o is p.Undef;
  o as p.Undef;
  <p.Undef>[];
  p.pi x;
}
class C {
  factory C.r() = p.Undef;
  C();
}
class D extends p.Undef {}
