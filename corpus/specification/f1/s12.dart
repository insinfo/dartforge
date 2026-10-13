class C {
  factory C.r() = Undef;
  C();
}
Undef v1 = throw 0;
void f(Object o) {
  try {} on Undef catch (e) {}
  o as Undef;
  o is Undef;
  o is! Undef;
  <Undef>[];
  List<Undef> l;
  new Undef();
  const Undef();
  Undef();
  Undef<int>();
  Undef.named();
  Undef<int>.named();
}
class D1 extends Undef {}
class D2 implements Undef {}
class D3 with Undef {}
mixin M on Undef {}
class D4 = Object with Undef;
class D5 = Undef with D1;
extension E on Undef {}
typedef T = Undef;
typedef F = void Function(Undef);
void g<X extends Undef>(Undef p, {Undef? q}) {}
