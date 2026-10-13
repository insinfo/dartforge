class A {
  int foo() => 0;
}
class A {
  String foo() => "";
  String bar() => "";
}
class C extends A {
  String foo() => "";
  int bar() => 0;
}
class D implements A {}
