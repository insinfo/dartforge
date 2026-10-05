class A {
  const A() : x = throw '';
  final int x;
}
class B {
  final int x;
  const B(int y) : x = y ~/ 0;
}
const b = const B(1);
