class A {
  final int x;
  const A(this.x);
}
class B extends A {
  const B(super.x);
  const B.b({int y = n}) : super(y);
  void m([int z = n, w = const A(n), List v = const [n], u = A(0), t = Indef]) {}
}
int n = 0;
var fn = ({int k = n}) => k;
void g() {
  void loc([int j = n]) {}
  loc();
}
typedef F = void Function([int i]);
