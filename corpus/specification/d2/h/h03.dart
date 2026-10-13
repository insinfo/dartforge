void f() {
  const E.foo();
  const E.v();
  const E.bar();
  const E();
  const B();
  const B.name();
  const B.zz();
}
enum E {
  v;
  void foo() {}
}
class A {
  const A.name();
}
typedef B = A;
