class A {
  final List<int> list = f();
  const factory A() = B;
}
class B implements A {
  final List<int> list = const [];
  const B();
}
List<int> f() {
  return [3];
}
