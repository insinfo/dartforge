class A {
  static void foo() {}
  void bar() {}
}

extension E on A {
  void foo() {}
  static void bar() {}
}
