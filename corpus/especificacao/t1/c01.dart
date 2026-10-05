class A {
  int foo() => 0;
}
class A {
  String bar() => "";
}
void f(A a) {
  a.foo();
  a.bar();
}
