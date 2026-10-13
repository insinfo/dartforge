int v = 0;
void fn() {}
int get gt => 0;
class C {
  factory C.r() = v;
  C();
}
v v1 = throw 0;
fn v2 = throw 0;
gt v3 = throw 0;
void f(Object o) {
  try {} on v catch (e) {}
  o as v;
  o is v;
  <v>[];
  new v();
  const v();
  new fn();
}
class D1 extends v {}
class D2 implements fn {}
class D3 with v {}
mixin M on v {}
