int f() => 1;
class A {
  final int a = f();
  final b = [1];
  final int c = 2;
  static final int s = f();
  int d = f();
  const A();
  const A.nome();
  const factory A.fac() = A;
}
enum E {
  v;
  final int a = f();
  const E();
}
