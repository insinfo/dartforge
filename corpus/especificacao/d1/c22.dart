class A {
  final int x;
  const A(String s) : x = s.length ~/ 0;
}
class B extends A {
  const B() : super('abc');
}
class C extends B {
  const C();
}
const c = const C();
