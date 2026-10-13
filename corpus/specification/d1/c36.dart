class A {
  final int x;
  final int x2;
  const A(this.x) : x = 1, x2 = 2;
}
const a = const A(1);
class B { final x = 1; final x = 2; const B(); }
const b = const B();
