class A {
  final int x;
  const A(int y) : x = y.foo;
}
class B extends A {
  const B(int y) : super(y);
}
const a = const A(1);
const b = const B(1);
