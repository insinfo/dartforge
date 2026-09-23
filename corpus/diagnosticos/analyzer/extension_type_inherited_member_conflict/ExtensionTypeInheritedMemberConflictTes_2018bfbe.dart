class A {
  void foo(int a) {}
}

class B {
  void foo(String a) {}
}

class C implements A, B {
  void foo(Object a) {}
}

extension type D(C it) implements A, B {
  void foo() {}
}
